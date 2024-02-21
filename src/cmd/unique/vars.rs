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
pub struct UniqueVarInfo;

impl VarProviderInfo for UniqueVarInfo {
    fn name(&self) -> &'static str {
        "Unique command variables"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(key => "The value of the unique key"),
            var_info!(n_duplicates => "The number of duplicate records sharing the same unique key"),
        ]
    }
}

/// Placeholder to use in formatted records, which will be replaced by the
/// number of duplicates when the number if known at the time ofwriting to output
pub static DUP_PLACEHOLDER: &[u8] = b"{__n_dup__}";

#[derive(Debug, Default)]
pub struct UniqueVars {
    key_id: Option<usize>,
    size_id: Option<usize>,
}

impl UniqueVars {
    pub fn needs_duplicates(&self) -> bool {
        self.size_id.is_some()
    }

    pub fn set(&mut self, key: &SimpleValue, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.key_id {
            let v = symbols.get_mut(var_id);
            match key {
                SimpleValue::Text(t) => v.inner_mut().set_text(t),
                SimpleValue::Number(n) => v.inner_mut().set_float(n.0),
                SimpleValue::None => v.set_none(),
            }
        }
        if let Some(var_id) = self.size_id.take() {
            // set the placeholder just once (will not change)
            symbols
                .get_mut(var_id)
                .inner_mut()
                .set_text(DUP_PLACEHOLDER);
        }
    }
}

impl VarProvider for UniqueVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &UniqueVarInfo
    }

    fn allow_nested(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let id = b.symbol_id();
        let ty = match var.name.as_str() {
            "key" => {
                self.key_id = Some(id);
                None
            }
            "n_duplicates" => {
                self.size_id = Some(id);
                Some(VarType::Int)
            }
            _ => unreachable!(),
        };
        Ok(ty)
    }

    fn has_vars(&self) -> bool {
        self.key_id.is_some() || self.size_id.is_some()
    }
}
