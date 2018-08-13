use std::collections::HashMap;
use std::f64::NAN;
use std::str;

use error::CliResult;
use exprtk_rs::{Expression, SymbolTable};
use io::Record;
use var::*;

use regex;

pub struct ExprHelp;

impl VarHelp for ExprHelp {
    fn name(&self) -> &'static str {
        "Math expressions"
    }
    fn usage(&self) -> &'static str {
        "{{<expression>}}"
    }
    fn desc(&self) -> Option<&'static str> {
        Some(
            "Math expressions with variables. Common operators and functions can be used \
             (+, -, *, /, %, ^, min, max, sqrt, abs, exp, trignometric functions, ...). \
             Boolean expressions are possible with common operators and keywords (and/or/not/...).\
             See http://www.partow.net/programming/exprtk/ and \
             https://github.com/ArashPartow/exprtk/blob/master/readme.txt for more information. \
             Math expressions are also used by the 'filter' command.",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Setting a GC content attribute as fraction instead of percentage",
                "st . -p gc={{s:gc / 100}} seqs.fa",
            ),
            (
                "Removing DNA sequences with more than 10% of ambiguous bases",
                "st filter 's:count:ATGC / s:seqlen >= 0.1' input.fa",
            ),
            (
                "Selecting IDs with a certain pattern:",
                "st filter \".id like 'AB*'\" input.fa",
            ),
            (
                "Selecting IDs from a list:",
                "st filter -uml id_list.txt 'def(l:1)' seqs.fa",
            ),
        ])
    }
}

lazy_static! {
    static ref VAR_RE: regex::Regex =
        regex::Regex::new(r"(\.?[A-Za-z][A-Za-z0-9_]*)(:[A-Za-z0-9][A-Za-z0-9\._]*)*").unwrap();
    static ref DEF: regex::Regex = regex::Regex::new(r"def\(\s*([A-Za-z0-9_:\.]+)\s*\)").unwrap();
}

#[derive(Debug)]
pub struct ExprVars {
    // expr_id, expression,
    exprs: Vec<(
        // expr_id
        usize,
        Expression,
        // [var_id -> ExprTk var_id]
        Vec<(usize, usize)>,
        // [var_id -> ExprTk string_id]
        Vec<(usize, usize)>,
        // def [var_id -> ExprTk var id]
        Vec<(usize, usize)>,
    )>,
}

impl ExprVars {
    pub fn new() -> CliResult<ExprVars> {
        Ok(ExprVars { exprs: vec![] })
    }
}

impl VarProvider for ExprVars {
    fn prefix(&self) -> Option<&str> {
        Some("expr_")
    }
    fn name(&self) -> &'static str {
        "expression"
    }

    fn register_var(
        &mut self,
        expr_string: &str,
        expr_id: usize,
        vars: &mut VarStore,
    ) -> CliResult<bool> {
        let mut symbols = SymbolTable::new();

        // def() function
        // 0/0 == NAN (c_double::NAN doesn't work)
        //symbols.add_func1("def", |var| if var == 0./0. { 0. } else { 1. })?;

        // def() function: not actually a function, just a variable that will be set
        let mut def = vec![];
        let expr_string = DEF.replace_all(expr_string, |m: &regex::Captures| {
            let name = m.get(1).unwrap().as_str().to_string();
            let name = if name.starts_with(".") {
                &name[1..]
            } else {
                &name
            };
            let v = format!("def_{}", name.replace(':', "_").replace('.', ""));
            let (var_id, _) = vars.register_var(name);
            let expr_var_id = symbols
                .add_variable(&v, 0.)
                .unwrap()
                .unwrap_or_else(|| symbols.get_var_id(&v).unwrap());
            def.push((var_id, expr_var_id));
            v
        });

        // replace variables with characters not accepted by the expression parser
        // keywords will also be replaced, however none of them contains "illegal"
        // characters, therefore nothing will happen to them.
        let mut string_ids = vec![];
        let mut replacements = HashMap::new();
        let expr_string = VAR_RE.replace_all(&expr_string, |m: &regex::Captures| {
            let var = m.get(0).unwrap().as_str().to_string();
            let new_name = var.replace(':', "_").replace('.', "");
            replacements
                .entry(new_name.clone())
                .or_insert_with(|| var.to_owned());
            new_name
        });

        // register strings
        for (new_name, var) in &replacements {
            if var.starts_with(".") {
                let (var_id, _) = vars.register_var(&var[1..]);
                if let Some(expr_var_id) = symbols.add_stringvar(&new_name, b"")? {
                    string_ids.push((var_id, expr_var_id));
                }
            }
        }

        // expression
        let (expr, expr_vars) = Expression::with_vars(&expr_string, symbols).map_err(|e| {
            if e.message
                .to_lowercase()
                .contains("invalid string operation")
            {
                // make error message more clear
                return "Invalid string operation in math expression. Is it possible that there \
                        are string variables without the '.' prefix (.variable)?"
                    .to_string();
            }
            format!("{}", e)
        })?;
        let mut var_ids = vec![];
        for (name, expr_var_id) in expr_vars {
            let orig_name = replacements.get(&name).unwrap_or(&name);
            let (var_id, _) = vars.register_var(orig_name);
            var_ids.push((var_id, expr_var_id));
        }

        self.exprs.push((expr_id, expr, var_ids, string_ids, def));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.exprs.is_empty()
    }

    fn set(&mut self, _: &Record, data: &mut Data) -> CliResult<()> {
        // copy values from symbol table to context
        for &mut (expr_id, ref mut expr, ref var_ids, ref string_ids, ref def) in &mut self.exprs {
            // scalars
            for &(var_id, expr_var_id) in var_ids {
                let value = data.symbols.get_float(var_id)?.unwrap_or(NAN);
                expr.symbols().set_value(expr_var_id, value);
            }
            // strings
            for &(var_id, expr_var_id) in string_ids {
                let s = data.symbols.get_text(var_id).unwrap_or(b"");
                expr.symbols().set_string(expr_var_id, s);
            }
            // def() "function"
            for &(var_id, expr_var_id) in def {
                let v = if data.symbols.is_empty(var_id) {
                    0.
                } else {
                    1.
                };
                expr.symbols().set_value(expr_var_id, v);
            }

            data.symbols.set_float(expr_id, expr.value());
        }
        Ok(())
    }
}
