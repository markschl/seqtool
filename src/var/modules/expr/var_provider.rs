use crate::io::{QualConverter, Record};
use crate::var::{
    attr::Attributes,
    func::Func,
    symbols::{SymbolTable, VarType},
    ArgInfo, VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::{CliError, CliResult};

use super::{code_or_file, Expr};

type Expressions = super::one_expr::Expressions<Expr>;

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
    fn info(&self) -> &dyn VarProviderInfo {
        &ExprInfo
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        assert_eq!(var.name, "____expr");
        let expr = var.arg(0);
        self.0.register_expr(expr, b)?;
        Ok(None)
    }

    fn allow_nested(&self) -> bool {
        false
    }

    fn has_vars(&self) -> bool {
        self.0.num_exprs() > 0
    }

    fn set(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        self.0.eval(symbols, record)
    }
}

#[derive(Debug)]
pub struct ExprInfo;

impl VarProviderInfo for ExprInfo {
    fn name(&self) -> &'static str {
        "Expressions (Javascript)"
    }

    fn desc(&self) -> Option<&'static str> {
        Some(
            "Expressions with variables, from simple mathematical operations to \
            to arbitrarily complicated JavaScript code. \
            Expressions are usually specified enclosed in { curly brackets }, \
            except for the 'filter' commands, where they are directly used. \
            Insteda of JavaScript code, it is possible to refer to a source \
            file using 'file:path.js'. \
            *Returned value*: For simple one-liner expressions, the value is \
            directly used. \
            More complex scripts with multiple statements (if/else, loops, etc.) \
            explicitly require a 'return' statement to return the value.",
        )
    }

    fn vars(&self) -> &[VarInfo] {
        &[VarInfo {
            name: "____expr",
            args: &[&[ArgInfo::Required("")]],
            description: "",
            hidden: true,
        }]
    }

    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Set the GC content as attribute in the header (e.g. >id gc=0.568), \
                however calculate a proportion instead of percentage",
                "st pass -a gc='{{ gc/100 }}' seqs.fasta"
            ),
            (
                "The GC content calculation can also be done differently",
                "st pass -a gc='{{ charcount(\"GC\")/seqlen }}' seqs.fasta"
            ),
            (
                "Keep only sequences with <1% ambiguous bases \
                (>=99% non-ambiguous) and with at least 100bp",
                "st filter 'charcount(\"ACGT\") / seqlen >= .99 && seqlen >= 100' input.fasta",
            ),
            (
                "Distribute sequences into different files where the name is \
                obtained using a more complex rule. Note the 'return' statments, \
                which are necessary here, since it is not a simple expression. \
                With even longer code, consider using an extra script and supplying \
                -o \"outdir/{{ file:code.js }}.fasta\" instead",
                "st split -po \"outdir/{{ if (id.startsWith('some_prefix_')) { return 'file_1' } return 'file_2' }}.fasta\" input.fasta",
            ),
        ])
    }
}
