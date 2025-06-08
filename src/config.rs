use std::cell::Cell;
use std::io;
use std::path::Path;

use crate::cli::CommonArgs;
use crate::context::SeqContext;
use crate::error::CliResult;
use crate::io::input::{self, get_seq_reader, thread_reader, InFormat, InputConfig, SeqReader};
use crate::io::output::{
    self, infer_out_format, OutFormat, OutputConfig, OutputOpts, SeqFormatter, WriteFinish,
};
use crate::io::{IoKind, QualFormat, Record};
use crate::var::{build::VarBuilder, modules::VarProvider, symbols::SymbolTable, VarOpts};

#[derive(Debug)]
pub struct Config {
    /// Configuration of all the input streams
    /// (I/O, sequence format)
    pub input_config: Vec<InputConfig>,
    /// Configuration of the main output
    /// (I/O, sequence format)
    pub output_config: OutputConfig,
    /// Options needed to create more output streams
    pub output_opts: (OutputOpts, output::FormatOpts),
    /// Variable/expression related options
    pub var_opts: VarOpts,
    // Context provided while reading along with individual sequence records.
    // Contains the symbol table
    pub ctx: SeqContext,
    // Number of currently registered variables
    n_vars: usize,
    // used to remember, whether the parsing already started
    started: Cell<bool>,
}

impl Config {
    pub fn new(args: &mut CommonArgs) -> CliResult<Self> {
        // input
        let input_config = args.get_input_cfg()?;

        // output
        let (out_kind, _out_opts, _out_fmt_opts) = args.get_output_opts()?;
        let mut main_out_opts = _out_opts.clone();
        let mut main_outmft_opts = _out_fmt_opts.clone();
        infer_out_format(
            out_kind.as_ref(),
            &input_config[0].format.format,
            &mut main_out_opts,
            &mut main_outmft_opts,
        );
        let out_format = OutFormat::from_opts(&main_outmft_opts)?;
        let output_config = OutputConfig {
            kind: out_kind,
            writer: main_out_opts,
            format: out_format,
        };

        // variables
        let var_opts: VarOpts = args.get_var_opts()?;

        // quality score format
        let qual_format = match input_config[0].format.format {
            InFormat::Fastq { format } => format,
            InFormat::FaQual { .. } => QualFormat::Phred,
            _ => QualFormat::Sanger,
        };

        // context used while reading
        let mut ctx = SeqContext::new(
            args.attr.attr_fmt.clone(),
            qual_format,
            output_config.writer.clone(),
        );
        ctx.init_vars(&var_opts, input_config[0].format.seqtype, &output_config)?;

        Ok(Self {
            input_config,
            output_config,
            output_opts: (_out_opts, _out_fmt_opts),
            var_opts,
            ctx,
            n_vars: 0,
            started: Cell::new(false),
        })
    }

    pub fn set_custom_varmodule(&mut self, provider: Box<dyn VarProvider>) -> CliResult<()> {
        self.ctx
            .set_custom_varmodule(provider, &self.var_opts, &self.output_config)
    }

    pub fn build_vars<F, O, E>(&mut self, mut action: F) -> Result<O, E>
    where
        F: FnMut(&mut VarBuilder) -> Result<O, E>,
    {
        let rv = {
            let mut builder = VarBuilder::new(
                &mut self.ctx.var_modules,
                &mut self.ctx.attrs,
                &mut self.n_vars,
            );
            action(&mut builder)
        };
        // done, grow the symbol table
        self.ctx.symbols.resize(self.n_vars);
        rv
    }

    /// Provides access to the custom `VarProvider` of the given type in a closure,
    /// if it is found. Panics otherwise.
    pub fn with_command_vars<M, O, E>(
        &mut self,
        func: impl FnOnce(Option<&mut M>, &mut SymbolTable) -> Result<O, E>,
    ) -> Result<O, E>
    where
        M: VarProvider + 'static,
    {
        self.ctx.custom_vars(func)
    }

    /// Returns a `FormatWriter` for the configured output format
    /// (via CLI or deduced from the output path).
    /// I/O writers are constructed separately, e.g. with io_writer_other().
    ///
    /// This function may register new variables.
    pub fn get_format_writer(&mut self) -> CliResult<Box<dyn SeqFormatter>> {
        // TODO: need to clone due to borrowing issues
        let fmt = self.output_config.format.clone();
        self.build_vars(|b| fmt.get_formatter(b))
    }

    /// Returns an I/O writer (of type WriteFinish) directly without any scope
    /// taking care of cleanup.
    /// The caller is responsible for invoking finish() on the writer when done.
    ///
    /// The returned writer may be a compressed writer if configured accordingly
    /// using CLI options or deduced from the output path extension.
    ///
    /// Writing in a background thread is not possible, since that
    /// would require a scoped function.
    pub fn io_writer<P>(&mut self, path: P) -> CliResult<Box<dyn WriteFinish>>
    where
        P: AsRef<Path>,
    {
        self.ctx.io_writer(path)
    }

    /// Provides an io Writer and `Vars` in a scope and takes care of cleanup (flushing)
    /// when done.
    pub fn with_io_writer<F, O>(self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write, Config) -> CliResult<O>,
    {
        let kind = self.output_config.kind.clone().unwrap_or(IoKind::Stdio);
        self.ctx.check_stdout(&kind)?;
        kind.with_thread_writer(&self.output_config.writer.clone(), |writer| {
            func(writer, self)
        })
    }

    /// Returns a new (`WriteFinish`, `FormatWriter`) pair
    /// (output format configured CLI or deduced from the output path).
    ///
    /// This function may register new variables (if the sequence format is different from
    /// the main output).
    ///
    /// The output is *not* reported to variable providers via `VarProvider::init_output()`,
    /// this only happens for the main output.
    ///
    // TODO: variables/attributes are double-registered, but this adds very little overhead
    pub fn new_output<I>(
        &mut self,
        kind: I,
    ) -> CliResult<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>
    where
        I: Into<IoKind>,
    {
        let kind = kind.into();
        self.ctx.check_stdout(&kind)?;
        let mut out_opts = self.output_opts.0.clone();
        let mut out_format_opts = self.output_opts.1.clone();
        infer_out_format(
            Some(&kind),
            &self.input_config[0].format.format,
            &mut out_opts,
            &mut out_format_opts,
        );
        let out_format = OutFormat::from_opts(&out_format_opts)?;

        let io_writer = kind.io_writer(&out_opts)?;
        let fmt_writer = self.build_vars(|b| out_format.get_formatter(b))?;
        Ok((io_writer, fmt_writer))
    }

    /// Provides a reader (reading input sequentially) within a context,
    /// and a `Vars` object for convenience (otherwise, two nested closures
    /// would be needed).
    pub fn read<F>(&mut self, mut func: F) -> CliResult<()>
    where
        F: FnMut(&dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.init_reader()?;
        for cfg in &self.input_config {
            thread_reader(&cfg.reader, |io_rdr| {
                self.ctx.init_input(cfg)?;
                input::read(io_rdr, &cfg.format, &mut |rec| {
                    self.ctx.set_record(&rec)?;
                    func(rec, &mut self.ctx)
                })
            })?;
        }
        Ok(())
    }

    /// Reads records of several readers alongside each other,
    /// whereby the record IDs should all match.
    /// The records cannot be provided at the same time in a slice,
    /// instead they are provided sequentially (cycling through the readers).
    /// The first argument is the reader number (0-based index),
    /// from which the record originates.
    ///
    /// `SeqContext::set_record()` needs to be called manually to handle
    /// variables in the output.
    pub fn read_alongside<F>(&mut self, id_check: bool, mut func: F) -> CliResult<()>
    where
        F: FnMut(usize, &dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.init_reader()?;
        input::read_alongside(&self.input_config, id_check, |i, rec| {
            func(i, rec, &mut self.ctx)
        })
    }

    /// Does some final preparation tasks regarding variables/functions before
    /// running the parser
    #[inline(never)]
    fn init_reader(&mut self) -> CliResult<()> {
        // remove unused modules
        self.ctx.filter_var_providers();
        // ensure that STDIN cannot be read twice
        // (would result in empty input on second attempt)
        if self.started.get() && self.has_stdin() {
            return fail!("Cannot read twice from STDIN");
        }
        self.started.set(true);
        Ok(())
    }

    pub fn read_parallel_init<Di, W, F, O>(
        &mut self,
        n_threads: u32,
        data_init: Di,
        work: W,
        mut func: F,
    ) -> CliResult<()>
    where
        W: Fn(&dyn Record, &mut O) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O, &mut SeqContext) -> CliResult<bool>,
        Di: Fn() -> O + Send + Sync,
        O: Send,
    {
        self.init_reader()?;
        for cfg in &self.input_config {
            thread_reader(&cfg.reader, |io_rdr| {
                self.ctx.init_input(cfg)?;
                input::read_parallel(
                    io_rdr,
                    n_threads,
                    &cfg.format,
                    &data_init,
                    &work,
                    |rec, out| {
                        self.ctx.set_record(rec)?;
                        func(rec, out, &mut self.ctx)
                    },
                )
            })?;
        }
        Ok(())
    }

    pub fn read_parallel<W, F, O>(&mut self, n_threads: u32, work: W, func: F) -> CliResult<()>
    where
        W: Fn(&dyn Record, &mut O) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O, &mut SeqContext) -> CliResult<bool>,
        O: Send + Default,
    {
        self.read_parallel_init(n_threads, Default::default, |rec, out| work(rec, out), func)
    }

    /// Returns the number of readers provided. Records are read
    /// sequentially (read, read_simple, read_parallel, etc.) or
    /// alongside each other (read_alongside)
    pub fn num_readers(&self) -> usize {
        self.input_config.len()
    }

    pub fn has_stdin(&self) -> bool {
        self.input_config
            .iter()
            .any(|cfg| cfg.reader.kind == IoKind::Stdio)
    }
}
