use std::borrow::Borrow;
use std::cell::Cell;
use std::io;

use crate::cli::{BasicStats, CommonArgs};
use crate::context::SeqContext;
use crate::error::CliResult;
use crate::io::input::{self, get_seq_reader, thread_reader, InFormat, InputConfig, SeqReader};
use crate::io::output::{
    self, infer_out_format, OutFormat, OutputConfig, OutputOpts, SeqFormatter, WriteFinish,
};
use crate::io::{IoKind, QualFormat, Record};
use crate::var::{build::VarBuilder, modules::VarProvider, VarOpts, VarProviders};

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
        let var_providers =
            VarProviders::new(&var_opts, input_config[0].format.seqtype, &output_config)?;
        let ctx = SeqContext::new(
            args.attr.attr_fmt.clone(),
            qual_format,
            var_providers,
            output_config.writer.clone(),
        );

        Ok(Self {
            input_config,
            output_config,
            output_opts: (_out_opts, _out_fmt_opts),
            var_opts,
            ctx,
            started: Cell::new(false),
        })
    }

    pub fn set_custom_varmodule(&mut self, provider: Box<dyn VarProvider>) -> CliResult<()> {
        self.ctx
            .var_providers
            .set_custom_varmodule(provider, &self.var_opts, &self.output_config)
    }

    pub fn build_vars<F, O>(&mut self, mut action: F) -> O
    where
        F: FnMut(&mut VarBuilder) -> O,
    {
        let d = &mut self.ctx.meta[0];
        self.ctx
            .var_providers
            .build(&mut d.attrs, &mut d.symbols, &mut action)
    }

    /// Gives access to the custom (command-specific) variable provider
    /// (assuming that it has been added),
    /// along with the mutable symtol table (from slot 0).
    pub fn with_custom_varmod<M, O>(&mut self, func: impl FnOnce(&mut M) -> O) -> O
    where
        M: VarProvider + 'static,
    {
        self.ctx.with_custom_varmod(0, |v, _| func(v)).unwrap()
    }

    /// Require a specified number of record metadata slots in `SeqContext::meta`
    /// (default 1). This may only be called once.
    ///
    /// Only specialized reading functions (currently `read2` and `read_alongside`)
    /// require manual handling of record metadata, for others (`read`, `read_parallel`),
    /// slot 0 is always used.
    pub fn require_meta_slots(&mut self, n: usize) {
        assert_eq!(self.ctx.meta.len(), 1);
        assert!(n > 1);
        self.ctx.meta.resize(n, self.ctx.meta[0].clone());
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
    pub fn io_writer<K>(&mut self, kind: K) -> CliResult<Box<dyn WriteFinish>>
    where
        K: Borrow<IoKind>,
    {
        self.ctx.io_writer(kind)
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

    /// Provides a reader (reading input sequentially) along with the SeqContext,
    /// **without** calling `SeqContext::set_record()`
    pub fn read_simple<F>(&mut self, mut func: F) -> CliResult<BasicStats>
    where
        F: FnMut(&dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.init_reader()?;
        for cfg in &self.input_config {
            thread_reader(&cfg.reader, |io_rdr| {
                self.ctx.init_input(cfg)?;
                input::read(io_rdr, &cfg.format, &mut |rec| {
                    self.ctx.increment_record();
                    func(rec, &mut self.ctx)
                })
            })?;
        }
        Ok(self.ctx.get_stats())
    }

    /// Provides a reader (reading input sequentially) along with the SeqContext.
    /// Beforehand,`SeqContext::set_record()` is called to set all variables.
    pub fn read<F>(&mut self, mut func: F) -> CliResult<BasicStats>
    where
        F: FnMut(&dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.init_reader()?;
        for cfg in &self.input_config {
            thread_reader(&cfg.reader, |io_rdr| {
                self.ctx.init_input(cfg)?;
                input::read(io_rdr, &cfg.format, &mut |rec| {
                    self.ctx.increment_record();
                    self.ctx.set_record(&rec, 0)?;
                    func(rec, &mut self.ctx)
                })
            })?;
        }
        Ok(self.ctx.get_stats())
    }

    /// Provides two readers, from which the records can be pulled independently
    ///
    /// Does not call `SeqContext::init_input()` and `SeqContext::set_record()`
    pub fn read2<F>(&mut self, mut scope: F) -> CliResult<BasicStats>
    where
        F: FnMut(&mut dyn SeqReader, &mut dyn SeqReader, &mut SeqContext) -> CliResult<()>,
    {
        self.init_reader()?;
        if self.input_config.len() != 2 {
            return fail!("Exactly two input files/streams are required",);
        }

        let cfg1 = &self.input_config[0];
        let cfg2 = &self.input_config[1];
        thread_reader(&cfg1.reader, |io_rdr1| {
            thread_reader(&cfg2.reader, |io_rdr2| {
                // self.ctx.init_input(in_opts1, seq_opts1)?;
                // self.ctx.init_input(in_opts2, seq_opts2)?;
                let mut rdr1 = get_seq_reader(io_rdr1, &cfg1.format)?;
                let mut rdr2 = get_seq_reader(io_rdr2, &cfg2.format)?;
                self.ctx.increment_record();
                scope(&mut rdr1, &mut rdr2, &mut self.ctx)
            })
        })?;
        Ok(self.ctx.get_stats())
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
    pub fn read_alongside<F>(&mut self, id_check: bool, mut func: F) -> CliResult<BasicStats>
    where
        F: FnMut(usize, &dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.init_reader()?;
        input::read_alongside(&self.input_config, id_check, |i, rec| {
            self.ctx.increment_record();
            func(i, rec, &mut self.ctx)
        })?;
        Ok(self.ctx.get_stats())
    }

    /// Does some final preparation tasks regarding variables/functions before
    /// running the parser
    #[inline(never)]
    fn init_reader(&mut self) -> CliResult<()> {
        // remove unused modules
        self.ctx.var_providers.clean_up();
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
    ) -> CliResult<BasicStats>
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
                        self.ctx.increment_record();
                        self.ctx.set_record(rec, 0)?;
                        func(rec, out, &mut self.ctx)
                    },
                )
            })?;
        }
        Ok(self.ctx.get_stats())
    }

    pub fn read_parallel<W, F, O>(
        &mut self,
        n_threads: u32,
        work: W,
        func: F,
    ) -> CliResult<BasicStats>
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
