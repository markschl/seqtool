use crate::error::CliResult;
use crate::io::Record;
use crate::var::{symbols::SymbolTable, VarBuilder};

use super::{replace_register_vars, ExprContext, Expression};

#[derive(Debug)]
pub struct Expressions<E: Expression> {
    expressions: Vec<(usize, E)>,
    // NOTE: context must come *after* expressions, since rquickjs expressions contain
    // Persistent<Atom>, which should not live longer than the context
    // TODO: always ok?
    context: E::Context,
}

impl<E: Expression> Expressions<E> {
    pub fn new(init_code: Option<&str>) -> CliResult<Self> {
        let mut context = E::Context::default();
        context.init(init_code)?;
        Ok(Self {
            expressions: vec![],
            context,
        })
    }

    pub fn register_expr(&mut self, code: &str, b: &mut VarBuilder) -> CliResult<()> {
        let (code, vars) = replace_register_vars(code, b)?;
        let mut expr = E::default();
        expr.register(b.symbol_id(), &code, &mut self.context)?;
        self.expressions.push((b.symbol_id(), expr));
        for var in vars {
            self.context.register(&var)?;
        }
        Ok(())
    }

    pub fn num_exprs(&self) -> usize {
        self.expressions.len()
    }

    pub fn eval(&mut self, symbols: &mut SymbolTable, record: &dyn Record) -> CliResult<()> {
        self.context.fill(symbols, record).map_err(|(_, msg)| msg)?;
        for (out_id, expr) in &mut self.expressions {
            expr.eval(symbols.get_mut(*out_id), &mut self.context)?;
        }
        Ok(())
    }
}
