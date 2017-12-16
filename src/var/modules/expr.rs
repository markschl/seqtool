use std::collections::HashMap;
use std::f64::NAN;

use std::str;

use io::Record;
use error::CliResult;
use var::*;

use meval;

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
            "Simple math expressions with variables. Common operators and functions can be used \
             (+, -, *, /, %, ^, min, max, sqrt, abs, exp, ln, trignometric functions, floor, ceil, \
             round, signum).",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Setting a GC content property as fraction instead of percentage",
                "seqtool . -p gc={{s:gc / 100}} seqs.fa",
            ),
            (
                "Summarise over the fraction of invalid bases (uppercase)",
                "seqtool count -k 'n:0.05:{{(s:seqlen - s:count:ACGTMRWSYKVHDBN) / s:seqlen}}'",
            ),
        ])
    }
}

lazy_static! {
    static ref VAR_RE: regex::Regex = regex::Regex::new(
        r"[A-Za-z][A-Za-z0-9_]*(:[A-Za-z0-9][A-Za-z0-9\._]*)+"
    ).unwrap();
}

#[derive(Debug)]
pub struct ExprVars {
    // var id -> varname
    vars: Vec<(usize, String)>,
    // variable context for Expressions
    ctx: HashMap<String, f64>,
    // expr_id, expression
    exprs: Vec<(usize, meval::Expr)>,
}

impl ExprVars {
    pub fn new() -> CliResult<ExprVars> {
        Ok(ExprVars {
            vars: vec![],
            ctx: HashMap::new(),
            exprs: vec![],
        })
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
        id: usize,
        vars: &mut VarStore,
    ) -> CliResult<bool> {
        // replace variables with characters not accepted by the expression parser
        let mut replacements = HashMap::new();
        let expr_string = VAR_RE.replace_all(expr_string, |m: &regex::Captures| {
            let var = m.get(0).unwrap().as_str().to_string();
            let rep = var.replace(':', "_").replace('.', "_");
            replacements
                .entry(rep.clone())
                .or_insert_with(|| var.to_owned());
            rep
        });

        let expr: meval::Expr = expr_string.parse()?;

        for v in expr.iter() {
            if let meval::tokenizer::Token::Var(ref name) = *v {
                let orig_name = replacements.get(name).unwrap_or(name);
                let (var_id, exists) = vars.register_var(orig_name);
                if !exists {
                    self.vars.push((var_id, name.to_string()));
                    self.ctx.insert(name.to_string(), 0.);
                }
            }
        }

        self.exprs.push((id, expr));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(&mut self, _: &Record, data: &mut Data) -> CliResult<()> {
        // copy values from symbol table to context
        for &(var_id, ref name) in &self.vars {
            let mut val = self.ctx.get_mut(name).expect("Bug: name should be present");
            *val = data.symbols.get_float(var_id)?.unwrap_or(NAN);
        }

        // calculate values
        for &(expr_id, ref expr) in &self.exprs {
            let value = expr.eval_with_context(&self.ctx)?;
            data.symbols.set_float(expr_id, value);
        }

        Ok(())
    }
}
