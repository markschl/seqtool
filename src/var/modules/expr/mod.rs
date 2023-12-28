use std::{fmt::Debug, str};

use crate::var::symbols::{SymbolTable, VarType};
use crate::{
    error::{CliError, CliResult},
    io::Record,
    var::{symbols::OptValue, *},
};

mod js;
// mod simple;
mod one_expr;
// mod two_exprs;
mod script;

use self::js as expr_mod;

// use self::two_exprs::Expressions;
type Expressions = self::one_expr::Expressions<expr_mod::Expr>;

pub use self::script::*;

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

    fn clear(&mut self) {}

    fn register(&mut self, _var: &Var) -> CliResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct ExprVars(Expressions);

impl ExprVars {
    pub fn new(init_code: Option<&str>) -> CliResult<Self> {
        let init_code = init_code
            .map(|c| Ok::<_, CliError>(code_or_file(c)?.to_string()))
            .transpose()?;
        Ok(Self(Expressions::new(init_code.as_deref())?))
    }
}

impl VarProvider for ExprVars {
    fn help(&self) -> &dyn VarHelp {
        &ExprHelp
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<Option<VarType>>> {
        if var.name != "____expr" || var.num_args() != 1 {
            return Ok(None);
        }
        let expr = var.arg_as::<String>(0).unwrap()?;
        self.0.register_expr(&expr, b)?;
        Ok(Some(None))
    }

    fn allow_dependent(&self) -> bool {
        false
    }

    fn has_vars(&self) -> bool {
        self.0.num_exprs() > 0
    }

    fn set(&mut self, record: &dyn Record, data: &mut MetaData) -> CliResult<()> {
        self.0.eval(&mut data.symbols, record)
    }
}

#[derive(Debug)]
pub struct ExprHelp;

impl VarHelp for ExprHelp {
    fn name(&self) -> &'static str {
        "Expressions (Javascript)"
    }
    fn usage(&self) -> Option<&'static str> {
        Some("{{ expression }}")
    }
    fn desc(&self) -> Option<&'static str> {
        Some(
            "Expressions with variables, from simple mathematical operations to \
            to arbitrarily complicated JavaScript code. \
            Expressions can be specified directly, or refer to a Javascript source \
            file using 'file:path.js'. \
            *Returned value*: For simple one-liner expressions, the value is \
            directly used.  \
            More complex scripts with multiple statements (if/else, loops, etc.) \
            explicitly require a 'return' statement to return the value.",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Set the GC content as attribute in the header (e.g. >id gc=0.568), \
                however calculate a proportion instead of percentage.",
                "st pass -a gc='{{ gc/100 }}' seqs.fasta"
            ),
            (
                "The GC content calculation can also be done differently:",
                "st pass -a gc='{{ charcount(\"GC\")/seqlen }}' seqs.fasta"
            ),
            (
                "Keep only sequences with <1% ambiguous bases \
                (>=99% non-ambiguous) and with at least 100bp.",
                "st filter 'charcount(\"ACGT\") / seqlen >= .99 && seqlen >= 100' input.fasta",
            ),
            (
                "Distribute sequences into different files where the name is \
                obtained using a more complex rule. Note the 'return' statments, \
                which are necessary here, since it is not a simple expression. \
                With even longer code, consider using an extra script and supplying \
                -o \"outdir/{{ file:code.js }}.fasta\" instead.",
                "st split -po \"outdir/{{ if (id.startsWith('some_prefix_')) { return 'file_1' } return 'file_2' }}.fasta\" input.fasta",
            ),
        ])
    }
}
