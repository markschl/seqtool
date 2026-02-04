use var_provider::{DynVarProviderInfo, VarType, dyn_var_provider};
use variable_enum_macro::variable_enum;

use crate::cmd::shared::key::Key;
use crate::var::{VarBuilder, modules::VarProvider, parser::Arg, symbols::SymbolTable};

variable_enum! {
    /// # Variables provided by the 'sort' command
    ///
    /// # Examples
    ///
    /// Sort by part of the sequence ID, which is obtained using
    /// a JavaScript expression.
    /// We additionally keep this substring by writing the sort key to a header
    /// attribute:
    ///
    /// `st sort -n '{ id.slice(2, 5) }' -a id_num='{num(key)}' input.fasta`
    ///
    /// >id001 id_num=1
    /// SEQ
    /// >id002 id_num=2
    /// SEQ
    /// (...)
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
