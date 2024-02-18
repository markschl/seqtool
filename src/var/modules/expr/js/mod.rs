use std::hash::{Hash, Hasher};

use fxhash::{FxHashMap, FxHasher64};

use crate::error::CliResult;
use crate::var::VarBuilder;

use super::{ExprContext, Expression, Var};

#[cfg(feature = "expr")]
mod expr;
mod parser;

#[cfg(feature = "expr")]
pub use self::expr::*;
pub use self::parser::*;

pub fn replace_register_vars(script: &str, b: &mut VarBuilder) -> CliResult<(String, Vec<Var>)> {
    let mut vars = FxHashMap::default();
    match parse_script(script) {
        Ok(ast) => {
            let new_code = ast
                .rewrite(|name, args| {
                    if b.has_var(name) {
                        let func = try_opt!(st_func_from_parsed(name, args, false));
                        b.register_nested_var(&func).transpose().map(|res| {
                            res.map(|(var_id, _, _)| {
                                // get unique placeholder variable name (function arguments are hashed)
                                let js_varname = if func.num_args() == 0 {
                                    func.name.clone()
                                } else {
                                    let mut hasher = FxHasher64::default();
                                    func.args.hash(&mut hasher);
                                    format!("{}_{:x}", func.name, hasher.finish())
                                };
                                vars.insert(var_id, js_varname.clone());
                                js_varname
                            })
                        })
                    } else {
                        None
                    }
                })
                .unwrap();
            // dbg!(ast, &new_code);
            Ok((
                new_code,
                vars.into_iter()
                    .map(|(symbol_id, name)| Var { symbol_id, name })
                    .collect(),
            ))
        }
        Err(mut rest) => {
            if rest.len() > 100 {
                rest.truncate(100);
                rest.push_str("...");
            }
            fail!(
                "Failed to parse expression. \
                There may be a syntax error or a literal /regex/ notation, which is not supported \
                (use `new RegExp(\"regex\")` instead). \
                The remaining code containing the problem: {}",
                rest
            )
        }
    }
}
