use crate::cmd::cmp::Category;
use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::cmd::shared::item::Key;

use crate::var::modules::VarProvider;
use crate::var::parser::Arg;
use crate::var::symbols::SymbolTable;
use crate::var::{VarBuilder, VarStore};

variable_enum! {
    /// # Variables/functions provided by the 'cmp' command
    ///
    /// # Examples
    ///
    /// Compare two files by ID and sequence hash and store all commonly found
    /// records in a new file (some statistics is printed to STDERR):
    ///
    /// `st cmp input1.fasta input2.fasta --common1 common.fasta`
    ///
    /// common  942
    /// unique1  51
    /// unique2  18
    CmpVar {
        /// Record category:
        /// 'common' (record present in both files based on comparison of keys),
        /// 'unique1' (record only in first file),
        /// or 'unique2' (record only in second file).
        Category(Text),
        /// Short category code: 'c' for common, 'u1' for unique1, 'u2' for unique2
        CategoryShort(Text),
        /// The value of the compared key
        Key(?),
    }
}

#[derive(Debug, Default)]
pub struct CmpVars {
    vars: VarStore<CmpVar>,
}

impl CmpVars {
    pub fn set(&mut self, key: &Key, cat: Category, symbols: &mut SymbolTable) {
        for (symbol_id, var) in self.vars.iter() {
            match var {
                CmpVar::Key => key.write_to_symbol(symbols.get_mut(*symbol_id)),
                CmpVar::Category => symbols
                    .get_mut(*symbol_id)
                    .inner_mut()
                    .set_text(cat.long_text().as_bytes()),
                CmpVar::CategoryShort => symbols
                    .get_mut(*symbol_id)
                    .inner_mut()
                    .set_text(cat.short_text().as_bytes()),
            }
        }
    }
}

impl VarProvider for CmpVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(CmpVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(CmpVar::from_func(name, args)?.map(|(var, out_type)| {
            let symbol_id = builder.store_register(var, &mut self.vars);
            (symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }
}
