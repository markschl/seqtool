use std::borrow::Cow;
use std::fmt::Debug;
use std::fs::read_to_string;

use crate::io::Record;
use crate::var::symbols::{OptValue, SymbolTable};

mod expressions;
pub mod js;
mod var_provider;

pub use self::var_provider::*;

#[derive(Debug, Default, Clone)]
pub struct Var {
    pub symbol_id: usize,
    pub name: String,
}

/// General trait used for registering/evaluating expressions, which
/// can be implemented for different expression engines
pub trait Expression: Default + Debug {
    type Context: ExprContext;

    fn register(
        &mut self,
        expr_id: usize,
        expr: &str,
        ctx: &mut Self::Context,
    ) -> Result<(), String>;

    fn eval(&mut self, out: &mut OptValue, ctx: &mut Self::Context) -> Result<(), String>;
}

pub trait ExprContext: Default {
    fn init(&mut self, init_code: Option<&str>) -> Result<(), String>;

    fn next_record(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> Result<(), (usize, String)>;

    // fn clear(&mut self) {}

    fn register(&mut self, _var: &Var) -> Result<(), String> {
        Ok(())
    }
}

pub fn code_or_file(expr: &str) -> Result<Cow<str>, String> {
    let expr = expr.trim();
    let prefix = "file:";
    #[allow(clippy::manual_strip)]
    if expr.starts_with(prefix) {
        let path = expr[prefix.len()..].trim_start();
        return read_to_string(path)
            .map(String::into)
            .map_err(|e| format!("Unable to read script file '{path}': {e}"));
    }
    Ok(expr.into())
}
