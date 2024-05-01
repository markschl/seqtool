use crate::helpers::DefaultHashMap as HashMap;
use crate::var::VarBuilder;

use self::parser::SimpleAst;

use super::{ExprContext, Expression, Var};

mod expr;
pub mod parser;

pub use self::expr::*;

pub fn replace_register_vars(
    ast: &SimpleAst,
    b: &mut VarBuilder,
) -> Result<(String, Vec<Var>), String> {
    let mut vars = HashMap::default();
    let new_code = ast.rewrite(|func| {
        b.register_var(func.name, func.args()).map(|res| {
            res.map(|(symbol_id, _)| {
                // get unique placeholder variable name (function arguments are hashed)
                let js_varname = if func.args().is_empty() {
                    func.name.to_string()
                } else {
                    format!("{}_{}", func.name, symbol_id)
                };
                vars.insert(symbol_id, js_varname.clone());
                js_varname
            })
        })
    })?;
    // dbg!(ast, &new_code);
    Ok((
        new_code,
        vars.into_iter()
            .map(|(symbol_id, name)| Var { symbol_id, name })
            .collect(),
    ))
}
