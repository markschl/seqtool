//! `Expressions` evaluates a list of expressions repeatedly using a single
//! engine.
//!
//! Currently, this is more of an an unnecessary wrapper, but the reason for this
//! being a separate module is that a more complicated evaluator featuring
//! two different engines (a fast and simple, and a slower JavaScript engine)
//! may be added in the future.

use var_provider::VarType;

use crate::io::Record;
use crate::var::{VarBuilder, symbols::SymbolTable};

use super::js::{parser::SimpleAst, replace_register_vars};
use super::{ExprContext, Expression};

#[derive(Debug)]
pub struct Expressions<E: Expression> {
    expressions: Vec<(usize, String, E)>,
    // NOTE: context must come *after* expressions, since rquickjs expressions contain
    // Persistent<Atom>, which should not live longer than the context
    // TODO: always ok?
    context: E::Context,
}

impl<E: Expression> Expressions<E> {
    pub fn new(init_code: Option<&str>) -> Result<Self, String> {
        let mut context = E::Context::default();
        context.init(init_code)?;
        Ok(Self {
            expressions: vec![],
            context,
        })
    }

    fn lookup(&self, script: &str) -> Option<usize> {
        self.expressions
            .iter()
            .find_map(|(id, code, _expr)| if script == code { Some(*id) } else { None })
    }

    pub fn register_expr(
        &mut self,
        ast: &SimpleAst,
        builder: &mut VarBuilder,
    ) -> Result<(usize, Option<VarType>), String> {
        if let Some(symbol_id) = self.lookup(ast.script) {
            Ok((symbol_id, None))
        } else {
            let mut expr = E::default();
            let (code, vars) = replace_register_vars(ast, builder)?;
            let symbol_id = builder.increment();
            expr.register(symbol_id, &code, &mut self.context)?;
            self.expressions.push((symbol_id, code, expr));
            for var in vars {
                self.context.register(&var)?;
            }
            Ok((symbol_id, None))
        }
    }

    pub fn num_exprs(&self) -> usize {
        self.expressions.len()
    }

    pub fn eval(&mut self, symbols: &mut SymbolTable, record: &dyn Record) -> Result<(), String> {
        self.context
            .next_record(symbols, record)
            .map_err(|(_, msg)| msg)?;
        for (out_id, _, expr) in &mut self.expressions {
            expr.eval(symbols.get_mut(*out_id), &mut self.context)?;
        }
        Ok(())
    }
}
