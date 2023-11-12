use std::cell::Cell;
use std::io;

use crate::error::CliResult;
use crate::io::*;
use crate::opt;
use crate::var;

#[derive(Debug)]
pub struct Config<'a> {
    input_opts: Vec<input::InputOptions>,
    output_opts: output::OutputOptions,
    var_opts: var::VarOpts<'a>,
    started: Cell<bool>,
}

impl<'a> Config<'a> {
    pub fn from_args(args: &'a opt::Args) -> CliResult<Config<'a>> {
        Self::new(args, None)
    }

    pub fn from_args_with_help(
        args: &'a opt::Args,
        custom_help: &dyn var::VarHelp,
    ) -> CliResult<Config<'a>> {
        Self::new(args, Some(custom_help))
    }

    pub fn new(
        args: &'a opt::Args,
        custom_help: Option<&dyn var::VarHelp>,
    ) -> CliResult<Config<'a>> {
        // initiate options

        let input_opts = args.get_input_opts()?;

        let out_opts = args.get_output_opts(Some(&input_opts[0].format))?;

        let var_opts = args.get_env_opts()?;

        if var_opts.var_help {
            let h = if let Some(h) = custom_help {
                format!("{}\n\n{}", h.format(), var::var_help())
            } else {
                var::var_help()
            };
            return fail!(h);
        }

        Ok(Config {
            output_opts: out_opts,
            input_opts,
            var_opts,
            started: Cell::new(false),
        })
    }

    pub fn input_opts(&self) -> &[input::InputOptions] {
        &self.input_opts
    }

    fn get_vars(&self) -> CliResult<var::Vars> {
        let mut vars = var::get_vars(&self.var_opts, &self.input_opts[0].format)?;
        vars.init_output(&self.output_opts)?;
        Ok(vars)
    }

    pub fn with_vars<F, O>(&self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut var::Vars) -> CliResult<O>,
    {
        let mut vars = self.get_vars()?;
        func(&mut vars)
    }

    pub fn writer<F, O>(&self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn output::Writer<&mut dyn io::Write>, &mut var::Vars) -> CliResult<O>,
    {
        self.with_vars(|v| {
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

    pub fn io_writer<F, O>(&self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write, &mut var::Vars) -> CliResult<O>,
    {
        output::io_writer(&self.output_opts, |writer| {
            let mut vars = self.get_vars()?;
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
        self.check_repetition()?;
        input::io_readers(&self.input_opts, |o, rdr| {
            input::run_reader(rdr, &o.format, o.cap, o.max_mem, &mut func)
        })?;
        Ok(())
    }

    pub fn read_alongside<F>(&self, func: F) -> CliResult<()>
    where
        F: FnMut(usize, &dyn Record) -> CliResult<()>,
    {
        self.check_repetition()?;
        input::read_alongside(&self.input_opts, func)
    }

    pub fn num_readers(&self) -> usize {
        self.input_opts.len()
    }

    pub fn read<F>(&self, vars: &mut var::Vars, mut func: F) -> CliResult<()>
    where
        F: FnMut(&dyn Record, &mut var::Vars) -> CliResult<bool>,
    {
        self.check_repetition()?;
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
        self.check_repetition()?;
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
        self.check_repetition()?;
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
            .any(|o| o.kind == input::InputType::Stdin)
    }

    /// ensures that STDIN cannot be read twice
    /// (would result in empty input on second attempt)
    fn check_repetition(&self) -> CliResult<()> {
        if self.started.get() && self.has_stdin() {
            return fail!("Cannot read twice from STDIN");
        }
        self.started.set(true);
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
