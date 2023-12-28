use std::cell::Cell;
use std::io;

use crate::error::CliResult;
use crate::io::output::{from_format, FormatWriter};
use crate::io::*;
use crate::opt::CommonArgs;
use crate::var::{self, VarProvider};

#[derive(Debug)]
pub struct Config {
    input_opts: Vec<input::InputOptions>,
    output_opts: output::OutputOptions,
    var_opts: var::VarOpts,
    started: Cell<bool>,
}

impl Config {
    pub fn new(args: &CommonArgs) -> CliResult<Config> {
        let input_opts = args.get_input_opts()?;

        let output_opts = args.get_output_opts(Some(&input_opts[0].format))?;

        let var_opts = args.get_var_opts()?;

        Ok(Config {
            output_opts,
            input_opts,
            var_opts,
            started: Cell::new(false),
        })
    }

    pub fn input_opts(&self) -> &[input::InputOptions] {
        &self.input_opts
    }

    // pub fn output_opts(&self) -> &output::OutputOptions {
    //     &self.output_opts
    // }

    // TODO: make private
    pub fn get_vars(&self, custom_mod: Option<Box<dyn VarProvider>>) -> CliResult<var::Vars> {
        let mut vars = var::get_vars(&self.var_opts, &self.input_opts[0].format, custom_mod)?;
        vars.init_output(&self.output_opts)?;
        Ok(vars)
    }

    pub fn with_vars<F, O>(&self, custom_mod: Option<Box<dyn VarProvider>>, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut var::Vars) -> CliResult<O>,
    {
        let mut vars = self.get_vars(custom_mod)?;
        func(&mut vars)
    }

    /// Provids a format writer, io writer and `Vars` object within a scope closure
    /// for simple use.
    /// The output format is deduced from the CLI options and/or the path.
    /// The IO writer may compress the data if configured accordingly (CLI options)
    /// or deduced from the extension.
    pub fn writer<F, O>(&self, func: F) -> CliResult<O>
    where
        F: FnOnce(
            &mut dyn output::FormatWriter,
            &mut dyn io::Write,
            &mut var::Vars,
        ) -> CliResult<O>,
    {
        self.writer_with_custom(None, func)
    }

    /// Like `writer`, but additionally takes a custom `VarProvider`
    pub fn writer_with_custom<F, O>(
        &self,
        custom_mod: Option<Box<dyn VarProvider>>,
        func: F,
    ) -> CliResult<O>
    where
        F: FnOnce(
            &mut dyn output::FormatWriter,
            &mut dyn io::Write,
            &mut var::Vars,
        ) -> CliResult<O>,
    {
        self.with_vars(custom_mod, |vars| {
            output::writer(&self.output_opts, vars, |writer, io_writer, vars| {
                func(writer, io_writer, vars)
            })
        })
    }

    /// Like `writer`, but additionally takes a `Vars` object that was already
    /// constructed outside (via `with_vars`).
    #[cfg_attr(not(feature = "find"), allow(unused))]
    pub fn writer_with_vars<F, O>(&self, vars: &mut var::Vars, func: F) -> CliResult<O>
    where
        F: FnOnce(
            &mut dyn output::FormatWriter,
            &mut dyn io::Write,
            &mut var::Vars,
        ) -> CliResult<O>,
    {
        output::writer(&self.output_opts, vars, |writer, io_writer, vars| {
            func(writer, io_writer, vars)
        })
    }

    /// Returns only a writer for the output format, as configured via the command
    /// line or deduced from the output path.
    /// Any IO writer should be constructed separately, e.g. with io_writer_other()
    pub fn format_writer(&self, vars: &mut var::Vars) -> CliResult<Box<dyn FormatWriter>> {
        from_format(&self.output_opts.format, vars)
    }

    /// Returns an io writer (of type WriteFinish) directly without any scope
    /// taking care of cleanup.
    /// This may be a compressed writer if configured accordingly using CLI options
    /// or deduced from the output path extension.
    /// The caller is thus responsible for calling finish() on the writer when done.
    pub fn io_writer_other(&self, path: &str) -> CliResult<Box<dyn output::WriteFinish>> {
        let mut o = self.output_opts.clone();
        o.kind = output::OutputKind::File(path.into());
        let io_writer = output::io_writer_from_kind(&o.kind)?;
        let out = output::compr_writer(io_writer, o.compression, o.compression_level)?;
        Ok(out)
    }

    /// Provides an io Writer and `Vars` in a scope and takes care of cleanup (flushing)
    /// when done. Takes an optional custom `VarProvider`.
    pub fn io_writer<F, O>(&self, custom_mod: Option<Box<dyn VarProvider>>, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write, &mut var::Vars) -> CliResult<O>,
    {
        output::io_writer(&self.output_opts, |writer| {
            let mut vars = self.get_vars(custom_mod)?;
            func(writer, &mut vars)
        })
    }

    /// Returns the number of readers provided. Records are read
    /// sequentially (read, read_simple, read_parallel, etc.) or
    /// alongside each other (read_alongside)
    pub fn num_readers(&self) -> usize {
        self.input_opts.len()
    }

    /// Provides a reader (reading input sequentially) within a context,
    /// and a `Vars` object for convenience (otherwise, two nested closures
    /// would be needed).
    pub fn read<F>(&self, vars: &mut var::Vars, mut func: F) -> CliResult<()>
    where
        F: FnMut(&dyn Record, &mut var::Vars) -> CliResult<bool>,
    {
        self._init_input()?;
        vars.finalize();
        input::io_readers(&self.input_opts, |o, rdr| {
            vars.new_input(o)?;
            input::run_reader(rdr, &o.format, o.cap, o.max_mem, &mut |rec| {
                vars.set_record(&rec)?;
                func(&rec, vars)
            })
        })?;
        Ok(())
    }

    /// Read without parsing any variable information for maximum
    /// performance. Usually used for counting records only, since
    /// writing records to output always requires variables.
    pub fn read_simple<F>(&self, mut func: F) -> CliResult<()>
    where
        F: FnMut(&dyn Record) -> CliResult<bool>,
    {
        self._init_input()?;
        input::io_readers(&self.input_opts, |o, rdr| {
            input::run_reader(rdr, &o.format, o.cap, o.max_mem, &mut func)
        })?;
        Ok(())
    }

    /// Reads records of several readers alongside each other,
    /// whereby the record IDs should all match.
    /// The records cannot be provided at the same time in a slice,
    /// instead they are provided sequentially (cycling through the readers).
    /// The first argument is the reader number (0-based index),
    /// from which the record originates.
    pub fn read_alongside<F>(&self, func: F) -> CliResult<()>
    where
        F: FnMut(usize, &dyn Record) -> CliResult<()>,
    {
        self._init_input()?;
        input::read_alongside(&self.input_opts, func)
    }

    pub fn read_parallel<W, F, O>(&self, n_threads: u32, work: W, mut func: F) -> CliResult<Vec<()>>
    where
        W: Fn(&dyn Record, &mut O) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O) -> CliResult<bool>,
        O: Send + Default,
    {
        self._init_input()?;
        input::io_readers(&self.input_opts, |o, rdr| {
            input::read_parallel(
                o,
                rdr,
                n_threads,
                || Ok(()),
                Default::default,
                |rec, out, _| work(rec, out),
                &mut func,
            )
        })
    }

    pub fn parallel_init_var<Si, S, Di, W, F, D>(
        &self,
        vars: &mut var::Vars,
        n_threads: u32,
        rset_init: Si,
        data_init: Di,
        work: W,
        mut func: F,
    ) -> CliResult<Vec<()>>
    where
        W: Fn(&dyn Record, &mut D, &mut S) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut D, &mut var::Vars) -> CliResult<bool>,
        Di: Fn() -> D + Send + Sync,
        D: Send,
        S: Send,
        Si: Fn() -> CliResult<S> + Send + Sync,
    {
        self._init_input()?;
        vars.finalize();
        input::io_readers(&self.input_opts, |in_opts, rdr| {
            vars.new_input(in_opts)?;
            input::read_parallel(
                in_opts,
                rdr,
                n_threads,
                &rset_init,
                &data_init,
                &work,
                |rec, out| {
                    vars.set_record(rec)?;
                    func(rec, out, vars)
                },
            )
        })
    }

    pub fn read_parallel_var<W, F, O>(
        &self,
        vars: &mut var::Vars,
        n_threads: u32,
        work: W,
        mut func: F,
    ) -> CliResult<Vec<()>>
    where
        W: Fn(&dyn Record, &mut O) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O, &mut var::Vars) -> CliResult<bool>,
        O: Send + Default,
    {
        self.read_parallel(n_threads, work, |rec, out| {
            vars.set_record(rec)?;
            func(rec, out, vars)
        })
    }

    pub fn has_stdin(&self) -> bool {
        self.input_opts
            .iter()
            .any(|o| o.kind == input::InputKind::Stdin)
    }

    #[inline(never)]
    fn _init_input(&self) -> CliResult<()> {
        // ensure that STDIN cannot be read twice
        // (would result in empty input on second attempt)
        // TODO: this is only a problem with the sample command
        if self.started.get() && self.has_stdin() {
            return fail!("Cannot read twice from STDIN");
        }
        self.started.set(true);
        // check if
        Ok(())
    }
}
