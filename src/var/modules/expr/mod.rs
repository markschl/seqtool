use std::borrow::Cow;
use std::fmt::Debug;
use std::fs::read_to_string;

use crate::error::{CliError, CliResult};
use crate::io::Record;
use crate::var::symbols::{OptValue, SymbolTable};

mod js;
mod one_expr;
#[cfg(feature = "expr")]
mod var_provider;

pub use self::js::*;
#[cfg(feature = "expr")]
pub use self::var_provider::*;

#[derive(Debug, Default, Clone)]
pub struct Var {
    pub symbol_id: usize,
    pub name: String,
}

pub trait Expression: Default + Debug {
    type Context: ExprContext;

    fn register(&mut self, expr_id: usize, expr: &str, ctx: &mut Self::Context) -> CliResult<()>;

    fn eval(&mut self, out: &mut OptValue, ctx: &mut Self::Context) -> CliResult<()>;
}

pub trait ExprContext: Default {
    fn init(&mut self, init_code: Option<&str>) -> CliResult<()>;

    fn fill(&mut self, symbols: &SymbolTable, record: &dyn Record)
        -> Result<(), (usize, CliError)>;

    // fn clear(&mut self) {}

    fn register(&mut self, _var: &Var) -> CliResult<()> {
        Ok(())
    }
}

pub fn code_or_file(expr: &str) -> CliResult<Cow<str>> {
    let expr = expr.trim();
    let prefix = "file:";
    #[allow(clippy::manual_strip)]
    if expr.starts_with(prefix) {
        return Ok(read_to_string(expr[prefix.len()..].trim_start()).map(String::into)?);
    }
    Ok(expr.into())
}
