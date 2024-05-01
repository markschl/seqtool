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
    /// arbitrarily complicated JavaScript code.
    ///
    /// Expressions are usually specified enclosed in { curly brackets }, except
    /// for the 'filter' commands, where they are directly used.
    ///
    /// Instead of JavaScript code, it is possible to refer to a source file
    /// using 'file:path.js'.
    ///
    /// *Returned value*: For simple one-liner expressions, the value is
    /// directly used.
    /// More complex scripts with multiple statements (if/else, loops, etc.)
    /// explicitly require a 'return' statement to return the value.
    ///
    /// # Examples
    ///
    /// Set the GC content as attribute in the header (e.g. >id gc=0.568),
    /// however calculate a proportion instead of percentage
    ///
    /// `st pass -a gc='{ gc/100 }' seqs.fasta`
    ///
    /// The GC content calculation can also be done differently
    ///
    /// `st pass -a gc='{ charcount("GC")/seqlen }' seqs.fasta`
    ///
    /// Keep only sequences with <1% ambiguous bases (>=99% non-ambiguous)
    /// and with at least 100bp
    ///
    /// `st filter 'charcount("ACGT") / seqlen >= 0.01 && seqlen >= 100' seqs.fasta`
    ///
    /// Distribute sequences into different files where the name is
    /// obtained using a more complex rule. Note the 'return' statments,
    /// which are necessary here, since it is not a simple expression.
    /// With even longer code, consider using an extra script and supplying
    /// -o "outdir/{ file:code.js }.fasta" instead
    ///
    /// `st split -po "outdir/{ if (id.startsWith('some_prefix_')) { return 'file_1' } return 'file_2' }.fasta" input.fasta`
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
