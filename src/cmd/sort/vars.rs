//! This module contains a `VarProvider` for a 'key' variable, which is
//! used by the 'sort' and 'unique' commands.

use crate::error::CliResult;
use crate::helpers::value::SimpleValue;
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
    pub fn set(&mut self, key: &SimpleValue, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            let v = symbols.get_mut(var_id);
            match key {
                SimpleValue::Text(t) => v.inner_mut().set_text(t),
                SimpleValue::Number(n) => v.inner_mut().set_float(n.0),
                SimpleValue::None => v.set_none(),
            }
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
