use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, HashMap},
    fmt::Debug,
    fs::read_to_string,
    hash::{Hash, Hasher},
    ops::Range,
    str,
};

use crate::var::varstring::parse_vars;
use crate::{error::CliResult, var::*};

use super::Var;

pub fn code_or_file(expr: &str) -> CliResult<Cow<str>> {
    let expr = expr.trim();
    #[allow(clippy::manual_strip)]
    if expr.starts_with("file:") {
        return Ok(read_to_string(&expr[5..]).map(String::into)?);
    }
    Ok(expr.into())
}

#[derive(Debug, Clone)]
struct ScriptEditor {
    expr_string: String,
    expr_replaced: String,
    prev_end: usize,
}

impl ScriptEditor {
    fn new(expr_string: &str) -> Self {
        Self {
            expr_string: expr_string.to_owned(),
            expr_replaced: String::with_capacity(expr_string.len()),
            prev_end: 0,
        }
    }

    fn replace_var(&mut self, rng: Range<usize>, func: &Func) -> String {
        let suffix = if func.num_args() == 0 {
            0
        } else {
            let mut hasher = DefaultHasher::default();
            func.args.hash(&mut hasher);
            hasher.finish()
        };
        let js_varname = format!("{}_{:x}", func.name, suffix);
        // TODO: bytes?
        self.expr_replaced
            .push_str(&self.expr_string[self.prev_end..rng.start]);
        self.expr_replaced.push_str(&js_varname);
        self.prev_end = rng.end;
        js_varname
    }

    fn finish(mut self) -> String {
        self.expr_replaced
            .push_str(&self.expr_string[self.prev_end..]);
        self.expr_replaced
    }
}

// replace variables with characters not accepted by the expression parser,
// simultaneously obtaining a list of them
pub fn replace_register_vars(expr: &str, b: &mut VarBuilder) -> CliResult<(String, Vec<Var>)> {
    let expr_string = code_or_file(expr)?.to_string();
    let mut editor = ScriptEditor::new(&expr_string);
    let mut vars = HashMap::new();
    for (rng, func) in parse_vars(&expr_string) {
        // exclude method calls and reserved words
        // println!("func {:?}", func);
        let byte_before = rng
            .0
            .start
            .checked_sub(1)
            .and_then(|i| expr_string.as_bytes().get(i));
        if byte_before != Some(&b'.')
        // && !RESERVED_WORDS.contains(&func.name)
        {
            // We try to register a variable/function
            if let Some((var_id, _, _)) = b.register_dependent_var(&func)? {
                let varname = editor.replace_var(rng.0, &func);
                vars.insert(var_id, varname);
                continue;
            }
        }
        // Function is not provided by seqtool:
        // In that case try registering every non-quoted argument,
        // which could again be a seqtool variable.
        // The regex in varstring.rs does not match functions with deeper
        // nesting, this code should thus detect every possible variable.
        for (arg, arg_rng) in func.args.iter().zip(&rng.1) {
            let f: Func = Func::var(arg.to_string());
            // println!("arg {:?} {:?}", arg_rng, f);
            if let Some((var_id, _, _)) = b.register_dependent_var(&f)? {
                let varname = editor.replace_var(arg_rng.clone(), &f);
                vars.insert(var_id, varname);
            }
        }
    }
    let expr_replaced = editor.finish();
    Ok((
        expr_replaced,
        vars.into_iter()
            .map(|(symbol_id, name)| Var { symbol_id, name })
            .collect(),
    ))
}
