use std::any::TypeId;
use std::cell::Cell;
use std::path::Path;

use crate::error::CliResult;
use crate::helpers::seqtype::SeqType;
use crate::io::input::InputConfig;
use crate::io::output::{OutputConfig, OutputOpts, WriteFinish};
use crate::io::{IoKind, QualConverter, QualFormat, Record};
use crate::var::{
    attr::{AttrFormat, Attributes},
    modules::VarProvider,
    symbols::SymbolTable,
    VarOpts,
};

/// Object providing access to variables/functions, header attributes and
/// methods to convert quality scores.
/// It should be initialized with each new sequence record, which will then
/// be passed to all `VarProvider`s, which will then in turn fill in the
/// symbol table with values for all necessary variables.
#[derive(Debug)]
pub struct SeqContext {
    // variable provider modules
    pub var_modules: Vec<Box<dyn VarProvider>>,
    // command-specific variable provider: (index in array, type ID)
    pub custom_module: Option<(usize, TypeId)>,
    // These fields are public in order to avoid borrowing issues
    // in some implementations.
    pub symbols: SymbolTable,
    pub attrs: Attributes,
    pub qual_converter: QualConverter,
    // needed by `io_writer_from_path`
    // TODO: another copy is in `Config` due to borrowing issues
    pub output_opts: OutputOpts,
    /// Set to `true` as soon as STDOUT is effectively used
    stdout_in_use: Cell<bool>,
}

impl SeqContext {
    pub fn new(attr_format: AttrFormat, qual_format: QualFormat, output_opts: OutputOpts) -> Self {
        Self {
            var_modules: Vec::new(),
            custom_module: None,
            symbols: SymbolTable::new(0),
            attrs: Attributes::new(attr_format),
            qual_converter: QualConverter::new(qual_format),
            output_opts,
            stdout_in_use: Cell::new(false),
        }
    }

    pub fn init_vars(
        &mut self,
        opts: &VarOpts,
        seqtype_hint: Option<SeqType>,
        out_cfg: &OutputConfig,
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
            .set_has_header(opts.meta_has_header),
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
            m.init_output(out_cfg)?;
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
    pub fn set_custom_varmodule(
        &mut self,
        module: Box<dyn VarProvider>,
        opts: &VarOpts,
        out_cfg: &OutputConfig,
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
            m.init_output(out_cfg)?;
        }
        Ok(())
    }

    /// Removes all variable providers that don't have any variables registered
    pub fn filter_var_providers(&mut self) {
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

    #[cold]
    pub fn check_stdout(&self, kind: &IoKind) -> CliResult<()> {
        if kind == &IoKind::Stdio {
            if self.stdout_in_use.get() {
                return fail!("Cannot write two different sources to STDOUT (-)");
            }
            self.stdout_in_use.set(true);
        }
        Ok(())
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
    pub fn io_writer<P>(&self, path: P) -> CliResult<Box<dyn WriteFinish>>
    where
        P: AsRef<Path>,
    {
        let kind = IoKind::from_path(path)?;
        self.check_stdout(&kind)?;
        let w = kind.io_writer(&self.output_opts)?;
        Ok(w)
    }

    /// Initialize context with a new input
    /// (done in Config while reading)
    pub fn init_input(&mut self, cfg: &InputConfig) -> CliResult<()> {
        for m in &mut self.var_modules {
            m.init_input(cfg)?;
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

    // pub fn record_data_clone(&self) -> (Attributes, SymbolTable) {
    //     (self.attrs.clone(), self.symbols.clone())
    // }

    /// Parse attributes into an external object and update an external symbols table
    /// (the `SeqContext` state does thus not except for internal states of `VarProvider` modules)
    pub fn set_record_with(
        &mut self,
        record: &dyn Record,
        attrs: &mut Attributes,
        symbols: &mut SymbolTable,
    ) -> CliResult<()> {
        if attrs.has_read_attrs() {
            attrs.parse(record);
        }
        for m in &mut self.var_modules {
            m.set_record(record, symbols, attrs, &mut self.qual_converter)?;
        }
        Ok(())
    }
}
