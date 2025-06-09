use std::{any::TypeId, io};

use var_provider::DynVarProviderInfo;

use crate::error::CliResult;
use crate::helpers::seqtype::SeqType;
use crate::io::{input::InputConfig, output::OutputConfig, QualConverter, Record};

use self::modules::{VarProvider, MODULE_INFO};
use self::{attr::Attributes, symbols::SymbolTable};

pub mod attr;
pub mod build;
pub mod modules;
pub mod parser;
pub mod symbols;
pub mod varstring;

pub use self::build::*;

#[derive(Debug, Clone)]
pub struct VarOpts {
    // metadata
    pub metadata_sources: Vec<String>,
    pub meta_delim_override: Option<u8>,
    pub meta_has_header: bool,
    pub meta_id_col: u32,
    pub meta_dup_ids: bool,
    // expressions
    pub expr_init: Option<String>,
}

#[cold]
pub fn print_var_help(
    custom_help: Option<Box<dyn DynVarProviderInfo>>,
    markdown: bool,
    command_only: bool,
) -> Result<(), io::Error> {
    if let Some(m) = custom_help {
        m.print_help(markdown)?;
    }
    if !command_only {
        for m in MODULE_INFO {
            m.print_help(markdown)?;
        }
    }
    Ok(())
}

/// The object holding all the variable providers
#[derive(Debug)]
pub struct VarProviders {
    // variable provider modules
    modules: Vec<Box<dyn VarProvider>>,
    // command-specific variable provider: (index in array, type ID)
    custom_mod: Option<(usize, TypeId)>,
    // Number of currently registered variables
    n_vars: usize,
}

impl VarProviders {
    pub fn new(
        opts: &VarOpts,
        seqtype_hint: Option<SeqType>,
        out_cfg: &OutputConfig,
    ) -> CliResult<Self> {
        // **note**: the module order is important!
        // Modules only have access to variables from preceding modules,
        // so only very simple dependencies (nested functions) are possible.
        // As a simple workaround, `set_custom_varmodule` appends
        // `ExprVars` and `CnvVars` a second time.
        let mut modules: Vec<Box<dyn VarProvider>> = vec![
            Box::new(
                modules::meta::MetaVars::new(
                    &opts.metadata_sources,
                    opts.meta_delim_override,
                    opts.meta_dup_ids,
                )?
                .set_id_col(opts.meta_id_col)
                .set_has_header(opts.meta_has_header),
            ),
            // other modules
            Box::new(modules::general::GeneralVars::new(seqtype_hint)),
            Box::new(modules::stats::StatVars::new()),
            Box::new(modules::attr::AttrVars::new()),
            #[cfg(feature = "expr")]
            Box::new(modules::expr::ExprVars::new(opts.expr_init.as_deref())?),
            Box::new(modules::cnv::CnvVars::new()),
        ];

        // make modules aware of output options
        for m in &mut modules {
            m.init_output(out_cfg)?;
        }
        Ok(Self {
            modules,
            custom_mod: None,
            n_vars: 0,
        })
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
        debug_assert!(self.custom_mod.is_none());
        use crate::var::modules::*;
        let n = self.modules.len();
        self.modules.push(module);
        self.custom_mod = Some((n, (self.modules.last().unwrap()).get_type_id()));
        // add another module for evaluating expressions in order to be able to
        // use variables from the newly added module in JS expressions
        #[cfg(feature = "expr")]
        self.modules
            .push(Box::new(expr::ExprVars::new(opts.expr_init.as_deref())?));
        // we also want to be able to post-process these variables, so we also add a
        // conversion module
        self.modules.push(Box::new(cnv::CnvVars::new()));
        // finally, also run 'init' for the newly added modules
        for m in self.modules.iter_mut().skip(n) {
            m.init_output(out_cfg)?;
        }
        Ok(())
    }

    /// Removes all variable providers that don't have any variables registered
    pub fn clean_up(&mut self) {
        self.modules = self.modules.drain(..).filter(|m| m.has_vars()).collect();
        // update the index of the custom module (if it is still there)
        if let Some((_, ty_id)) = self.custom_mod {
            self.custom_mod = self
                .modules
                .iter()
                .position(|m| m.get_type_id() == ty_id)
                .map(|i| (i, ty_id));
        }
    }

    /// Provides mutable access to a the custom `VarProvider` of the given type
    /// (assuming it has been added with `set_custom_varmodule()`).
    /// Panics on type mismatch.
    pub fn custom_vars<M>(&mut self) -> Option<&mut M>
    where
        M: VarProvider + 'static,
    {
        self.custom_mod.map(|(i, _)| {
            self.modules[i]
                .as_mut()
                .as_any_mut()
                .downcast_mut::<M>()
                .unwrap()
        })
    }

    /// Provides a `VarBuilder` in a closure for registering variables,
    /// and resizes the provided symbol table when done.
    pub fn build<F, O>(
        &mut self,
        attrs: &mut Attributes,
        symbols: &mut SymbolTable,
        mut action: F,
    ) -> O
    where
        F: FnMut(&mut VarBuilder) -> O,
    {
        let rv = {
            let mut builder = VarBuilder::new(&mut self.modules, attrs, &mut self.n_vars);
            action(&mut builder)
        };
        // done, grow the symbol table
        symbols.resize(self.n_vars);
        rv
    }

    /// Register a new input with the VarProvider modules
    pub fn init_input(&mut self, cfg: &InputConfig) -> CliResult<()> {
        for m in &mut self.modules {
            m.init_input(cfg)?;
        }
        Ok(())
    }

    /// Update the symbol table with a new record
    #[inline(always)]
    pub fn update_symbols(
        &mut self,
        record: &dyn Record,
        out: &mut SymbolTable,
        attrs: &Attributes,
        qc: &mut QualConverter,
    ) -> CliResult<()> {
        for m in &mut self.modules {
            m.set_record(record, out, attrs, qc)?;
        }
        Ok(())
    }
}
