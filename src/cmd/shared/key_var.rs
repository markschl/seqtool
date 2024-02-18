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
pub struct KeyVarHelp;

impl VarProviderInfo for KeyVarHelp {
    fn name(&self) -> &'static str {
        "Sort command variables"
    }

    fn vars(&self) -> &[VarInfo] {
        &[var_info!(key => "The value of the key")]
    }
}

#[derive(Debug, Default)]
pub struct KeyVars {
    id: Option<usize>,
}

impl KeyVars {
    pub fn set(&mut self, key: &SimpleValue, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.id {
            let v = symbols.get_mut(var_id);
            match key {
                SimpleValue::Text(t) => v.inner_mut().set_text(t),
                SimpleValue::Number(n) => v.inner_mut().set_float(n.0),
                SimpleValue::None => v.set_none(),
            }
        }
    }
}

impl VarProvider for KeyVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &KeyVarHelp
    }

    fn allow_nested(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        assert_eq!(var.name, "key");
        self.id = Some(b.symbol_id());
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.id.is_some()
    }
}
