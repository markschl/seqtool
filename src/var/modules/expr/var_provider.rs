use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::io::{QualConverter, Record};
use crate::var::{
    attr::Attributes, modules::VarProvider, parser::Arg, symbols::SymbolTable, VarBuilder,
};

use super::code_or_file;
use super::js::{parser::Expression, JsExpr};

type Expressions = super::expressions::Expressions<JsExpr>;

variable_enum! {
    /// # Expressions (JavaScript)
    ///
    /// Expressions with variables, from simple mathematical operations to
    /// arbitrarily complex JavaScript code.
    ///
    /// Expressions are always enclosed in { curly brackets }. These brackets
    /// are optional for simple variables/functions in some cases,
    /// but mandatory for expressions.
    /// In addition, the 'filter' command takes an expression (without { brackets }).
    ///
    ///
    /// Instead of JavaScript code, it is possible to refer to a source file
    /// using 'file:path.js'.
    ///
    ///
    /// *Returned value*: For simple one-liner expressions, the value is
    /// directly used.
    /// More complex scripts with multiple statements (if/else, loops, etc.)
    /// explicitly require a `return` statement to return the value.
    ///
    ///
    /// # Examples
    ///
    /// Calculate the number of ambiguous bases in a set of DNA sequences and
    /// add the result as an attribute (ambig=...) to the header
    ///
    /// `st pass -a ambig='{seqlen - charcount("ACGT")}' seqs.fasta`
    ///
    /// >id1 ambig=3
    /// TCNTTAWTAACCTGATTAN
    /// >id2 ambig=0
    /// GGAGGATCCGAGCG
    /// (...)
    ///
    ///
    /// Discard sequences with >1% ambiguous bases or sequences shorter than 100bp
    ///
    /// `st filter 'charcount("ACGT") / seqlen >= 0.99 && seqlen >= 100' seqs.fasta`
    ///
    ///
    /// Distribute sequences into different files by a slightly complicated condition.
    /// Note the 'return' statments are are necessary here, since this is not a simple expression.
    /// With even longer code, consider using an extra script and supplying
    /// -o "outdir/{file:code.js}.fasta" instead
    ///
    /// `st split -po "outdir/{ if (id.startsWith('some_prefix_')) { return 'file_1' } return 'file_2' }.fasta" input.fasta`
    ///
    /// There should be two files now (`ls file_*.fasta`):
    /// file_1.fasta
    /// file_2.fasta
    ExprVar<'a> {
        #[hidden]
        ____Expr(?) { expr: Expression<'a> },
    }
}

#[derive(Debug)]
pub struct ExprVars(Expressions);

impl ExprVars {
    pub fn new(init_code: Option<&str>) -> Result<Self, String> {
        let init_code = init_code
            .map(|c| Ok::<_, String>(code_or_file(c)?.to_string()))
            .transpose()?;
        Ok(Self(Expressions::new(init_code.as_deref())?))
    }
}

impl VarProvider for ExprVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(ExprVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        if let Some((var, _)) = ExprVar::from_func(name, args)? {
            let ExprVar::____Expr { expr } = var;
            return expr
                .with_tree(|ast| Ok(Some(self.0.register_expr(ast, builder)?)))
                .and_then(|res| res);
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.0.num_exprs() > 0
    }

    fn set_record(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        self.0.eval(symbols, record)
    }
}
