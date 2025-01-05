use std::any::TypeId;
use std::cell::Cell;
use std::io;

use crate::cli::CommonArgs;
use crate::error::CliResult;
use crate::helpers::seqtype::SeqType;
use crate::io::{
    input::{self, InFormat, InputKind, InputOptions},
    output::{
        compr_writer, from_format, io_writer_from_kind, with_io_writer, FormatWriter, OutputKind,
        OutputOptions, WriteFinish,
    },
    Compression, QualConverter, QualFormat, Record,
};
use crate::var::{
    attr::{AttrFormat, Attributes},
    build::VarBuilder,
    modules::VarProvider,
    symbols::SymbolTable,
    VarOpts,
};

#[derive(Debug)]
pub struct Config {
    input_opts: Vec<InputOptions>,
    output_opts: OutputOptions,
    var_opts: VarOpts,
    // context provided along with individual sequence records,
    // contains the symbol table
    ctx: SeqContext,
    // number of registered variables
    n_vars: usize,
    // used to remember, whether the parsing already started
    started: Cell<bool>,
}

impl Config {
    pub fn new(args: &CommonArgs) -> CliResult<Self> {
        let input_opts = args.get_input_opts()?;
        let output_opts = args.get_output_opts(Some(&input_opts[0].format))?;
        let var_opts: VarOpts = args.get_var_opts()?;

        // quality score format
        let qual_format = match input_opts[0].format {
            InFormat::Fastq { format } => format,
            InFormat::FaQual { .. } => QualFormat::Phred,
            _ => QualFormat::Sanger,
        };

        // context used while reading
        let mut ctx = SeqContext::new(
            args.attr.attr_fmt.clone(),
            qual_format,
            (output_opts.compression, output_opts.compression_level),
            output_opts.append,
        );
        ctx.init_vars(&var_opts, input_opts[0].seqtype, &output_opts)?;

        Ok(Self {
            output_opts,
            input_opts,
            var_opts,
            ctx,
            n_vars: 0,
            started: Cell::new(false),
        })
    }

    pub fn input_opts(&self) -> &[InputOptions] {
        &self.input_opts
    }

    pub fn get_seqtype(&self) -> Option<SeqType> {
        self.input_opts[0].seqtype
    }

    // pub fn output_opts(&self) -> &OutputOptions {
    //     &self.output_opts
    // }

    pub fn set_custom_varmodule(&mut self, provider: Box<dyn VarProvider>) -> CliResult<()> {
        self.ctx
            .set_custom_varmodule(provider, &self.var_opts, &self.output_opts)
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
    /// This function can be used to obtain a `FormatWriter` within a
    /// `Config::build()` closure.
    pub fn get_format_writer(&mut self) -> CliResult<Box<dyn FormatWriter>> {
        let fmt = self.output_opts.format.clone();
        self.build_vars(|b| from_format(&fmt, b))
    }

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
        self.init_reader()?;
        input::with_io_readers(&self.input_opts, |o, rdr| {
            self.ctx.init_input(o)?;
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
        self.init_reader()?;
        input::read_alongside(&self.input_opts, |i, rec| func(i, rec, &mut self.ctx))
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
        self.init_reader()?;
        input::with_io_readers(&self.input_opts, |in_opts, rdr| {
            self.ctx.init_input(in_opts)?;
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
    // variable provider modules
    var_modules: Vec<Box<dyn VarProvider>>,
    // command-specific variable provider: (index in array, type ID)
    custom_module: Option<(usize, TypeId)>,
    // These fields are public in order to avoid borrowing issues
    // in some implementations.
    pub symbols: SymbolTable,
    pub attrs: Attributes,
    pub qual_converter: QualConverter,
    // needed by `io_writer_from_path`
    // TODO: this is duplicated data
    out_compression: (Compression, Option<u8>),
    append_out: bool,
}

impl SeqContext {
    pub fn new(
        attr_format: AttrFormat,
        qual_format: QualFormat,
        out_compression: (Compression, Option<u8>),
        append_out: bool,
    ) -> Self {
        Self {
            var_modules: Vec::new(),
            custom_module: None,
            symbols: SymbolTable::new(0),
            attrs: Attributes::new(attr_format),
            qual_converter: QualConverter::new(qual_format),
            out_compression,
            append_out,
        }
    }

    fn init_vars(
        &mut self,
        opts: &VarOpts,
        seqtype_hint: Option<SeqType>,
        out_opts: &OutputOptions,
    ) -> CliResult<()> {
        use crate::var::modules::*;
        // metadata lists
        self.var_modules.push(Box::new(
            meta::MetaVars::new(
                &opts.metadata_sources,
                opts.meta_delim_override,
                opts.meta_dup_ids,
            )?
            .set_id_col(opts.meta_id_col)
            .set_has_header(opts.has_header),
        ));

        // other modules
        self.var_modules
            .push(Box::new(general::GeneralVars::new(seqtype_hint)));
        self.var_modules.push(Box::new(stats::StatVars::new()));
        self.var_modules.push(Box::new(attr::AttrVars::new()));

        #[cfg(feature = "expr")]
        self.var_modules
            .push(Box::new(expr::ExprVars::new(opts.expr_init.as_deref())?));

        self.var_modules.push(Box::new(cnv::CnvVars::new()));

        // make modules aware of output options
        for m in &mut self.var_modules {
            m.init_output(out_opts)?;
        }
        Ok(())
    }

    /// Adds another custom variable provider module. It can be later accessed
    /// using `custom_vars()`. Calling this again with another module does not
    /// invalidate the previous one, but `custom_vars` will then return only the
    /// last-added module.
    ///
    /// In order to allow using variables from that module in expressions,
    /// we append another expression evaluation and a conversion module.
    fn set_custom_varmodule(
        &mut self,
        module: Box<dyn VarProvider>,
        opts: &VarOpts,
        out_opts: &OutputOptions,
    ) -> CliResult<()> {
        debug_assert!(self.custom_module.is_none());
        use crate::var::modules::*;
        let n = self.var_modules.len();
        self.var_modules.push(module);
        self.custom_module = Some((n, (self.var_modules.last().unwrap()).get_type_id()));
        // add another module for evaluating expressions in order to be able to
        // use variables from the newly added module in JS expressions
        #[cfg(feature = "expr")]
        self.var_modules
            .push(Box::new(expr::ExprVars::new(opts.expr_init.as_deref())?));
        // we also want to be able to post-process these variables, so we also add a
        // conversion module
        self.var_modules.push(Box::new(cnv::CnvVars::new()));
        // finally, also run 'init' for the newly added modules
        for m in self.var_modules.iter_mut().skip(n) {
            m.init_output(out_opts)?;
        }
        Ok(())
    }

    /// Removes all variable providers that don't have any variables registered
    fn filter_var_providers(&mut self) {
        self.var_modules = self
            .var_modules
            .drain(..)
            .filter(|m| m.has_vars())
            .collect();
        // update the index of the custom module (if it is still there)
        if let Some((_, ty_id)) = self.custom_module {
            self.custom_module = self
                .var_modules
                .iter()
                .position(|m| m.get_type_id() == ty_id)
                .map(|i| (i, ty_id));
        }
        // dbg!("filtered", &self.var_modules);
    }

    /// Provides access to the custom `VarProvider` of the given type in a closure,
    /// if it is found. Panics otherwise.
    pub fn custom_vars<M, O, E>(
        &mut self,
        func: impl FnOnce(Option<&mut M>, &mut SymbolTable) -> Result<O, E>,
    ) -> Result<O, E>
    where
        M: VarProvider + 'static,
    {
        let m = self.custom_module.map(|(i, _)| {
            self.var_modules[i]
                .as_mut()
                .as_any_mut()
                .downcast_mut::<M>()
                .unwrap()
        });
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
        let io_writer = io_writer_from_kind(&OutputKind::File(path.into()), self.append_out)?;
        let (compr, level) = self.out_compression;
        let out = compr_writer(io_writer, compr, level)?;
        Ok(out)
    }

    /// Initialize context with a new input
    /// (done in Config while reading)
    pub fn init_input(&mut self, in_opts: &InputOptions) -> CliResult<()> {
        for m in &mut self.var_modules {
            m.init_input(in_opts)?;
        }
        Ok(())
    }

    /// Initialize context with a new sequence record
    /// (done in Config while reading, or manually with Config::read_alongside)
    #[inline(always)]
    pub fn set_record(&mut self, record: &dyn Record) -> CliResult<()> {
        if self.attrs.has_read_attrs() {
            self.attrs.parse(record);
        }
        for m in &mut self.var_modules {
            m.set_record(
                record,
                &mut self.symbols,
                &mut self.attrs,
                &mut self.qual_converter,
            )?;
        }
        Ok(())
    }
}
