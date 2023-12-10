use std::cell::Cell;
use std::io;

use crate::error::CliResult;
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

    pub fn writer<F, O>(&self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn output::Writer<&mut dyn io::Write>, &mut var::Vars) -> CliResult<O>,
    {
        self.writer_with_custom(None, func)
    }

    pub fn writer_with_custom<F, O>(
        &self,
        custom_mod: Option<Box<dyn VarProvider>>,
        func: F,
    ) -> CliResult<O>
    where
        F: FnOnce(&mut dyn output::Writer<&mut dyn io::Write>, &mut var::Vars) -> CliResult<O>,
    {
        self.with_vars(custom_mod, |v| {
            output::writer(&self.output_opts, |writer| {
                v.build(|b| writer.register_vars(b))?;
                func(writer, v)
            })
        })
    }

    pub fn writer_with<F, O>(&self, vars: &mut var::Vars, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn output::Writer<&mut dyn io::Write>, &mut var::Vars) -> CliResult<O>,
    {
        output::writer(&self.output_opts, |writer| {
            vars.build(|b| writer.register_vars(b))?;
            func(writer, vars)
        })
    }

    pub fn io_writer<F, O>(&self, custom_mod: Option<Box<dyn VarProvider>>, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write, &mut var::Vars) -> CliResult<O>,
    {
        output::io_writer(&self.output_opts, |writer| {
            let mut vars = self.get_vars(custom_mod)?;
            func(writer, &mut vars)
        })
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

    pub fn read_alongside<F>(&self, func: F) -> CliResult<()>
    where
        F: FnMut(usize, &dyn Record) -> CliResult<()>,
    {
        self._init_input()?;
        input::read_alongside(&self.input_opts, func)
    }

    pub fn num_readers(&self) -> usize {
        self.input_opts.len()
    }

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

    pub fn parallel<W, F, O>(&self, n_threads: u32, work: W, mut func: F) -> CliResult<Vec<()>>
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

    pub fn parallel_var<W, F, O>(
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
        self.parallel(n_threads, work, |rec, out| {
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
        // TODO: this is only a problem with sample command
        if self.started.get() && self.has_stdin() {
            return fail!("Cannot read twice from STDIN");
        }
        self.started.set(true);
        // check if
        Ok(())
    }

    pub fn other_writer<'c>(
        &self,
        path: &str,
        vars: Option<&mut var::Vars>,
    ) -> CliResult<Box<dyn output::Writer<Box<dyn output::WriteFinish>> + 'c>> {
        let mut o = self.output_opts.clone();
        o.kind = output::OutputKind::File(path.into());
        let io_writer = output::io_writer_from_kind(&o.kind)?;
        let io_writer = output::compr_writer(io_writer, o.compression, o.compression_level)?;
        let mut w = output::from_format(io_writer, &o.format)?;
        if let Some(v) = vars {
            v.build(|b| w.register_vars(b))?;
        }
        Ok(w)
    }
}
