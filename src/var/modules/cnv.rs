use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::io::{QualConverter, Record};
use crate::var::{attr::Attributes, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

use super::VarProvider;

variable_enum! {
    /// # Type conversion
    ///
    /// # Examples
    ///
    /// Summarizing by a numeric header attribute in the form '>id n=3'
    ///
    /// `st count -k 'num(attr(n))' seqs.fa`
    CnvVar<'a> {
        /// Converts any expression or value to a number. Missing
        /// (undefined/null) values are left as-is.
        Num(Number) { expression: Arg<'a> },
    }
}

#[derive(Debug, Default)]
pub struct CnvVars {
    // list of:
    // (target ID, source ID)
    // whereby 'source' is the non-converted value and target is the converted
    // value, which we store in a new symbol
    num_vars: VarStore<usize>,
    // fixed strings to convert
    fixed_vars: VarStore<f64>,
}

impl CnvVars {
    pub fn new() -> CnvVars {
        Self::default()
    }
}

impl VarProvider for CnvVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(CnvVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        if let Some((var, out_type)) = CnvVar::from_func(name, args)? {
            let CnvVar::Num { expression } = var;
            match expression {
                Arg::Func(func) => {
                    if let Some((source_id, _)) = builder.register_var(func.name, func.args())? {
                        let target_id = builder.store_register(source_id, &mut self.num_vars);
                        return Ok(Some((target_id, out_type)));
                    } else {
                        // if unknown, we issue an error
                        return Err(format!(
                            "Unknown variable/function passed to 'Num': {}",
                            func.name
                        ));
                    }
                }
                Arg::Str(s) => {
                    let num = s
                        .parse()
                        .map_err(|_| format!("Cannot convert '{}' to a number", s))?;
                    let target_id = builder.store_register(num, &mut self.fixed_vars);
                    return Ok(Some((target_id, out_type)));
                }
                #[cfg(feature = "expr")]
                Arg::Expr(_) => unreachable!(),
            };
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        !self.num_vars.is_empty()
    }

    fn set_record(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        // set fixed-string numbers (only once)
        for (target_id, num) in self.fixed_vars.drain(..) {
            symbols.get_mut(target_id).inner_mut().set_float(num);
        }
        // dynamic conversion
        for (target_id, source_id) in self.num_vars.iter().cloned() {
            if let Some(val) = symbols.get(source_id).inner() {
                let num = val.get_float(rec)?;
                symbols.get_mut(target_id).inner_mut().set_float(num);
            } else {
                symbols.get_mut(target_id).set_none();
            }
        }
        Ok(())
    }
}
