use std::any::{Any, TypeId};
use std::fmt::Debug;

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};

use crate::helpers::any::AsAnyMut;
use crate::io::{input::InputConfig, output::OutputConfig, QualConverter, Record};

use super::attr::Attributes;
use super::parser::Arg;
use super::symbols::SymbolTable;
use super::VarBuilder;

pub mod attr;
pub mod cnv;
#[cfg(feature = "expr")]
pub mod expr;
pub mod general;
pub mod meta;
pub mod stats;

/// List of all variable/function provider modules,
/// used to generate the help pages
/// (independently of the variable provider modules themselves)
pub const MODULE_INFO: &[&dyn DynVarProviderInfo] = &[
    &dyn_var_provider!(general::GeneralVar),
    &dyn_var_provider!(stats::StatVar),
    &dyn_var_provider!(attr::AttrVar),
    &dyn_var_provider!(meta::MetaVar),
    #[cfg(feature = "expr")]
    &dyn_var_provider!(expr::ExprVar),
    &dyn_var_provider!(cnv::CnvVar),
];

/// *The* trait for variable/function provider modules.
pub trait VarProvider: Debug + AsAnyMut {
    fn info(&self) -> &dyn DynVarProviderInfo;

    /// Tries registering a variable / function with a module
    /// and returns `Some(VarType)` or `None` if the type is unknown beforehand.
    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String>;

    fn has_vars(&self) -> bool;

    /// Supplies a new record to the variable provider and expects it to
    /// update the symbol table with the variable values.
    fn set_record(
        &mut self,
        _rec: &dyn Record,
        _sym: &mut SymbolTable,
        _attr: &mut Attributes,
        _qc: &mut QualConverter,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Called on every new output stream (STDOUT or file).
    /// Some variable providers may need the information.
    /// Additional output files created using `Config::new_output()` are
    /// *not* provided here.
    fn init_output(&mut self, _: &OutputConfig) -> Result<(), String> {
        Ok(())
    }

    /// Called on every new input (STDIN or file).
    /// Some variable providers may need the information.
    fn init_input(&mut self, _: &InputConfig) -> Result<(), String> {
        Ok(())
    }

    /// Returns the type ID of the given concrete type
    /// (used for identifying custom variable providers in `Ctx`)
    fn get_type_id(&self) -> TypeId
    where
        Self: 'static,
    {
        self.type_id()
    }
}
