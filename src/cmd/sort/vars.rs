//! This module contains a `VarProvider` for a 'key' variable, which is
//! used by the 'sort' and 'unique' commands.

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::cmd::shared::sort_item::Key;
use crate::var::{modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder};

variable_enum! {
    /// # Sort command variables
    ///
    /// # Examples
    ///
    /// Sort sequences by their length and store the length in the sequence
    /// header in the sequence header, producing headers like this one:
    /// '>id1 seqlen=210'
    ///
    /// `st sort seqlen -a seqlen='{key}' input.fasta > output.fasta`
    SortVar {
        /// The value of the key used for sorting
        Key(?),
    }
}

#[derive(Debug, Default)]
pub struct SortVars {
    key_id: Option<usize>,
}

impl SortVars {
    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            key.write_to_symbol(symbols.get_mut(var_id));
        }
    }
}

impl VarProvider for SortVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(SortVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(SortVar::from_func(name, args)?.map(|(var, out_type)| {
            let SortVar::Key = var;
            let symbol_id = self.key_id.get_or_insert_with(|| builder.increment());
            (*symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        self.key_id.is_some()
    }
}
