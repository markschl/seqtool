use std::cell::Cell;
use std::io;

use fxhash::FxHashMap;

use crate::cli::CommonArgs;
use crate::error::CliResult;
use crate::io::{
    input::{self, InFormat, InputKind, InputOptions},
    output::{
        compr_writer, from_format, io_writer_from_kind, with_io_writer, FormatWriter, OutputKind,
        OutputOptions, WriteFinish,
    },
    Compression, QualConverter, QualFormat, Record, SeqAttr,
};
use crate::var::{
    attr::Attrs,
    func::Func,
    init_vars,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarOpts, VarProvider,
};

#[derive(Debug)]
pub struct Config {
    input_opts: Vec<InputOptions>,
    output_opts: OutputOptions,
    ctx: SeqContext,
    var_map: FxHashMap<Func, (usize, Option<VarType>, bool)>,
    attr_map: FxHashMap<String, usize>,
    started: Cell<bool>,
}

impl Config {
    pub fn new(args: &CommonArgs) -> CliResult<Self> {
        Self::with_vars(args, None)
    }

    pub fn with_vars(
        args: &CommonArgs,
        command_vars: Option<Box<dyn VarProvider>>,
    ) -> CliResult<Self> {
        let input_opts = args.get_input_opts()?;
        let output_opts = args.get_output_opts(Some(&input_opts[0].format))?;
        let var_opts: VarOpts = args.get_var_opts()?;

        // variable providers
        let mut var_modules = Vec::new();
        init_vars(&mut var_modules, command_vars, &var_opts, &output_opts)?;

        // // if --var-help requested, exit
        // if var_opts.var_help {
        //     for m in &var_modules {
        //         eprintln!("{}", m.help());
        //     }
        //     exit(2);
        // }

        // Where are attributes (key=value) appended?
        let append_attr = if var_opts.attr_opts.delim == ' ' {
            SeqAttr::Desc
        } else {
            SeqAttr::Id
        };

        // quality score format
        let qual_format = match input_opts[0].format {
            InFormat::Fastq { format } => format,
            InFormat::FaQual { .. } => QualFormat::Phred,
            _ => QualFormat::Sanger,
        };

        // context used while reading
        let ctx = SeqContext::new(
            var_modules,
            args.attr.adelim as u8,
            args.attr.aval_delim as u8,
            append_attr,
            qual_format,
            (output_opts.compression, output_opts.compression_level),
        );

        Ok(Self {
            output_opts,
            input_opts,
            ctx,
            var_map: FxHashMap::default(),
            attr_map: FxHashMap::default(),
            started: Cell::new(false),
        })
    }

    pub fn input_opts(&self) -> &[InputOptions] {
        &self.input_opts
    }

    // pub fn output_opts(&self) -> &OutputOptions {
    //     &self.output_opts
    // }

    pub fn build_vars<F, O>(&mut self, mut action: F) -> CliResult<O>
    where
        F: FnMut(&mut VarBuilder) -> CliResult<O>,
    {
        let rv = {
            let mut builder = VarBuilder::new(
                &mut self.ctx.var_modules,
                &mut self.var_map,
                &mut self.attr_map,
                &mut self.ctx.attrs,
            );
            action(&mut builder)
        };
        // done, grow the symbol table
        self.ctx.symbols.resize(self.var_map.len());
        rv
    }

    /// Provides access to the custom `VarProvider` of the given type in a closure,
    /// if it is found. Panics otherwise.
    pub fn with_command_vars<M, O>(
        &mut self,
        func: impl FnOnce(Option<&mut M>, &mut SymbolTable) -> CliResult<O>,
    ) -> CliResult<O>
    where
        M: VarProvider + 'static,
    {
        self.ctx.command_vars(func)
    }

    /// Returns a `FormatWriter` for the configured output format
    /// (via CLI or deduced from the output path).
    /// I/O writers are constructed separately, e.g. with io_writer_other().
    /// This function can be used to obtain a `FormatWriter` within a
    /// `Config::build()` closure.
    pub fn get_format_writer(&mut self) -> CliResult<Box<dyn FormatWriter>> {
        let fmt = self.output_opts.format.clone();
        self.build_vars(|b| from_format(&fmt, b))
    }

    // /// Provides a format writer and an I/O writer within a scope closure.
    // /// The output format is deduced from the CLI options and/or the path.
    // /// The IO writer may compress the data if configured accordingly (CLI options)
    // /// or deduced from the extension.
    // pub fn with_writer<F, O>(&self, func: F) -> CliResult<O>
    // where
    //     F: FnOnce(&mut dyn FormatWriter, &mut dyn io::Write) -> CliResult<O>,
    // {
    //     with_io_writer(&self.output_opts, |io_writer| {
    //         let mut format_writer = self.get_format_writer()?;
    //         func(&mut format_writer, io_writer)
    //     })
    // }

    /// Provides an io Writer and `Vars` in a scope and takes care of cleanup (flushing)
    /// when done.
    pub fn with_io_writer<F, O>(self, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write, Config) -> CliResult<O>,
    {
        with_io_writer(&self.output_opts.clone(), |writer| func(writer, self))
    }

    /// Provides a reader (reading input sequentially) within a context,
    /// and a `Vars` object for convenience (otherwise, two nested closures
    /// would be needed).
    pub fn read<F>(&mut self, mut func: F) -> CliResult<()>
    where
        F: FnMut(&dyn Record, &mut SeqContext) -> CliResult<bool>,
    {
        self.ctx._filter_var_modules();
        self._init_input()?;
        input::with_io_readers(&self.input_opts, |o, rdr| {
            self.ctx.new_input(o)?;
            input::run_reader(rdr, &o.format, o.cap, o.max_mem, &mut |rec| {
                self.ctx.set_record(&rec)?;
                func(rec, &mut self.ctx)
            })
        })?;
        Ok(())
    }

    /// Reads records of several readers alongside each other,
    /// whereby the record IDs should all match.
    /// The records cannot be provided at the same time in a slice,
    /// instead they are provided sequentially (cycling through the readers).
    /// The first argument is the reader number (0-based index),
    /// from which the record originates.
    pub fn read_alongside<F>(&mut self, mut func: F) -> CliResult<()>
    where
        F: FnMut(usize, &dyn Record, &mut SeqContext) -> CliResult<()>,
    {
        self._init_input()?;
        input::read_alongside(&self.input_opts, |i, rec| func(i, rec, &mut self.ctx))
    }

    #[inline(never)]
    fn _init_input(&self) -> CliResult<()> {
        // ensure that STDIN cannot be read twice
        // (would result in empty input on second attempt)
        if self.started.get() && self.has_stdin() {
            return fail!("Cannot read twice from STDIN");
        }
        self.started.set(true);
        Ok(())
    }

    pub fn read_parallel_init<Si, D, Di, W, F, O>(
        &mut self,
        n_threads: u32,
        rset_init: Si,
        data_init: Di,
        work: W,
        mut func: F,
    ) -> CliResult<Vec<()>>
    where
        W: Fn(&dyn Record, &mut O, &mut D) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O, &mut SeqContext) -> CliResult<bool>,
        Di: Fn() -> O + Send + Sync,
        O: Send,
        D: Send,
        Si: Fn() -> CliResult<D> + Send + Sync,
    {
        self._init_input()?;
        input::with_io_readers(&self.input_opts, |in_opts, rdr| {
            self.ctx.new_input(in_opts)?;
            input::read_parallel(
                in_opts,
                rdr,
                n_threads,
                &rset_init,
                &data_init,
                &work,
                |rec, out| {
                    self.ctx.set_record(rec)?;
                    func(rec, out, &mut self.ctx)
                },
            )
        })
    }

    pub fn read_parallel<W, F, O>(&mut self, n_threads: u32, work: W, func: F) -> CliResult<Vec<()>>
    where
        W: Fn(&dyn Record, &mut O) -> CliResult<()> + Send + Sync,
        F: FnMut(&dyn Record, &mut O, &mut SeqContext) -> CliResult<bool>,
        O: Send + Default,
    {
        self.read_parallel_init(
            n_threads,
            || Ok(()),
            Default::default,
            |rec, out, _| work(rec, out),
            func,
        )
    }

    /// Returns the number of readers provided. Records are read
    /// sequentially (read, read_simple, read_parallel, etc.) or
    /// alongside each other (read_alongside)
    pub fn num_readers(&self) -> usize {
        self.input_opts.len()
    }

    pub fn has_stdin(&self) -> bool {
        self.input_opts.iter().any(|o| o.kind == InputKind::Stdin)
    }
}

/// Object providing access to variables/functions, header attributes and
/// methods to convert quality scores.
/// It should be initialized with each new sequence record, which will then
/// be passed to all `VarProvider`s, which will then in turn fill in the
/// symbol table with values for all necessary variables.
#[derive(Debug)]
pub struct SeqContext {
    var_modules: Vec<Box<dyn VarProvider>>,
    // these fields are public in order to avoid borrowing issues
    pub symbols: SymbolTable,
    pub attrs: Attrs,
    pub qual_converter: QualConverter,
    // needed by `io_writer_from_path`
    // TODO: this is duplicated data
    out_compression: (Compression, Option<u8>),
}

impl SeqContext {
    pub fn new(
        var_modules: Vec<Box<dyn VarProvider>>,
        attr_delim: u8,
        attr_value_delim: u8,
        append_attr: SeqAttr,
        qual_format: QualFormat,
        out_compression: (Compression, Option<u8>),
    ) -> Self {
        Self {
            var_modules,
            symbols: SymbolTable::new(0),
            attrs: Attrs::new(attr_delim, attr_value_delim, append_attr),
            qual_converter: QualConverter::new(qual_format),
            out_compression,
        }
    }

    #[inline]
    fn _filter_var_modules(&mut self) {
        // remove unused modules
        self.var_modules = self
            .var_modules
            .drain(..)
            .filter(|m| m.has_vars())
            .collect();
        // println!("vars final {:?}", self);
    }

    /// Provides access to the custom `VarProvider` of the given type in a closure,
    /// if it is found. Panics otherwise.
    pub fn command_vars<M, O>(
        &mut self,
        func: impl FnOnce(Option<&mut M>, &mut SymbolTable) -> CliResult<O>,
    ) -> CliResult<O>
    where
        M: VarProvider + 'static,
    {
        let m = self.var_modules.first_mut().unwrap();
        let m = m.as_mut().as_any_mut().downcast_mut::<M>();
        func(m, &mut self.symbols)
    }

    /// Returns an I/O writer (of type WriteFinish) directly without any scope
    /// taking care of cleanup.
    /// The caller is responsible for invoking finish() on the writer when done.
    /// The returned writer may be a compressed writer if configured accordingly
    /// using CLI options or deduced from the output path extension.
    /// Writing in a background thread is not possible, since that
    /// would require a scoped function.
    ///
    /// This method is part of `Context` and not `Config` because it is only
    /// needed in cases where multiple writers have to be dynamically constructed
    /// while running the reader. To use the 'standard' configured output, use
    /// `Config::with_io_writer`.
    ///
    // TODO: this is mostly useful for the 'split' command, which always has the
    // same output format and compression settings. It may not be flexible enough
    // for all future uses.
    pub fn io_writer_from_path(&self, path: &str) -> CliResult<Box<dyn WriteFinish>> {
        let io_writer = io_writer_from_kind(&OutputKind::File(path.into()))?;
        let (compr, level) = self.out_compression;
        let out = compr_writer(io_writer, compr, level)?;
        Ok(out)
    }

    /// Initialize context with a new input
    /// (done in Config while reading)
    pub fn new_input(&mut self, in_opts: &InputOptions) -> CliResult<()> {
        for m in &mut self.var_modules {
            m.new_input(in_opts)?;
        }
        Ok(())
    }

    /// Initialize context with a new sequence record
    /// (done in Config while reading, or manually with Config::read_alongside)
    #[inline(always)]
    pub fn set_record(&mut self, record: &dyn Record) -> CliResult<()> {
        if self.attrs.has_attrs() {
            let (id, desc) = record.id_desc_bytes();
            self.attrs.parse(id, desc);
        }
        for m in &mut self.var_modules {
            m.set(
                record,
                &mut self.symbols,
                &mut self.attrs,
                &mut self.qual_converter,
            )?;
        }
        Ok(())
    }
}
