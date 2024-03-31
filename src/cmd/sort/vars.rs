//! This module contains a `VarProvider` for a 'key' variable, which is
//! used by the 'sort' and 'unique' commands.

use crate::cmd::shared::sort_item::Key;
use crate::error::CliResult;
use crate::var::{
    func::Func,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct SortVarInfo;

impl VarProviderInfo for SortVarInfo {
    fn name(&self) -> &'static str {
        "Sort command variables"
    }

    fn vars(&self) -> &[VarInfo] {
        &[var_info!(key => "The value of the key used for sorting")]
    }
}

#[derive(Debug, Default)]
pub struct SortVars {
    key_id: Option<usize>,
}

impl SortVars {
    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            key.into_symbol(symbols.get_mut(var_id));
        }
    }
}

impl VarProvider for SortVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &SortVarInfo
    }

    fn allow_nested(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        assert_eq!(var.name, "key");
        self.key_id = Some(b.symbol_id());
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.key_id.is_some()
    }
}
