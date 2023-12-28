use crate::error::CliResult;
use crate::var::{
    symbols::{SymbolTable, VarType},
    Func, VarBuilder, VarHelp, VarProvider,
};

use super::Key;

#[derive(Debug)]
pub struct KeyVarHelp;

impl VarHelp for KeyVarHelp {
    fn name(&self) -> &'static str {
        "Sort command variables"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "key",
            "The value of the key (-k/--key argument). \
            The default key is the sequence.",
        )])
    }
}

#[derive(Debug, Default)]
pub struct KeyVars {
    id: Option<usize>,
}

impl KeyVars {
    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        if let Some(var_id) = self.id {
            let v = symbols.get_mut(var_id);
            match key {
                Key::Text(t) => v.inner_mut().set_text(t),
                Key::Numeric(n) => v.inner_mut().set_float(n.0),
                Key::None => v.set_none(),
            }
        }
    }
}

impl VarProvider for KeyVars {
    fn help(&self) -> &dyn VarHelp {
        &KeyVarHelp
    }

    fn allow_dependent(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<Option<VarType>>> {
        if var.name == "key" {
            var.ensure_no_args()?;
            self.id = Some(b.symbol_id());
            return Ok(Some(None));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.id.is_some()
    }
}
