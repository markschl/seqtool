use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::helpers::number::bin;
use crate::io::{QualConverter, Record};
use crate::var::{
    attr::Attributes,
    parser::Arg,
    symbols::{SymbolTable, Value},
    VarBuilder, VarStore,
};

use super::VarProvider;

variable_enum! {
    /// # Data conversion and processing
    ///
    /// # Examples
    ///
    /// Summarizing by a numeric header attribute in the form '>id n=3'
    ///
    /// `st count -k 'num(attr(n))' seqs.fa`
    ///
    /// Summarizing the distribution of the GC content in a set of DNA sequences
    /// in 5% intervals
    ///
    /// `st count -k 'bin(gc, 5)' seqs.fa`
    CnvVar<'a> {
        /// Converts any expression or value to a decimal number. Missing
        /// (undefined/null) values are left as-is.
        Num(Number) { expression: Arg<'a> },
        // /// Converts any expression or value to a whole number (integer).
        // /// In some cases, it may be desirable to use this function instead of
        // /// 'num(...)' to improve precision and/or performance.
        // /// Missing (undefined/null) values are left as-is.
        // Int(Number) { expression: Arg<'a> },
        /// Groups a continuous numeric number into discrete bins
        /// with a given interval. The intervals are represented as
        /// '(start, end]', whereby start <= value < end; the intervals
        /// are thus open on the left as indicated by '(', and closed
        /// on the right, as indicated by ']'.
        /// If not interval is given, a default width of 1 is assumed.
        Bin(Text) { expression: Arg<'a>, interval: f64 = 1. },
    }
}

// TODO: conversion to integer not activated because it is not straightforward:
// should we ignore decimals or throw an error if any present
// Is int() even of any use at some place?
// The JS function:
// function int(x) {
//     let i = parseInt(x);
//     if (isNaN(i)) {
//         if (x === undefined) return undefined;
//         if (x === null) return null;
//         throw `Could not convert '${x}' to an integer`;
//     }
//     if (typeof x === "string" && x.includes(".")) {
//         throw `Could not convert '${x}' to an integer`;
//     }
//     return i;
// }

#[derive(Debug, Clone, PartialEq)]
enum CnvVarType {
    Num,
    // Int,
    Bin(f64),
}

/// This is a macro because implementing it as method of CnvVarType would
/// lead to borrowing issues
macro_rules! value_to_symbol {
    ($self:expr, $val:expr, $target_id:expr, $symbols:expr, $rec:expr) => {{
        match $self {
            CnvVarType::Num => {
                let f = $val.get_float($rec)?;
                $symbols.get_mut($target_id).inner_mut().set_float(f)
            }
            // CnvVarType::Int => {
            //     let i = $val.get_int($rec)?;
            //     $symbols.get_mut($target_id).inner_mut().set_int(i)
            // }
            CnvVarType::Bin(ref interval) => {
                let num = $val.get_float($rec)?;
                $symbols
                    .get_mut($target_id)
                    .inner_mut()
                    .set_interval(bin(num, *interval));
            }
        }
        Ok::<(), String>(())
    }};
}

impl CnvVarType {
    fn write_to_symbol(
        &self,
        source_id: usize,
        target_id: usize,
        symbols: &mut SymbolTable,
        rec: &dyn Record,
    ) -> Result<(), String> {
        if let Some(val) = symbols.get(source_id).inner() {
            value_to_symbol!(self, val, target_id, symbols, rec)?;
        } else {
            symbols.get_mut(target_id).set_none();
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CnvVars {
    // list of:
    // (source ID, var type)
    // whereby 'source' is the non-converted value and target is the converted
    // value, which we store in a new symbol
    num_vars: VarStore<(usize, CnvVarType)>,
    // fixed strings to convert
    fixed_vars: VarStore<(String, CnvVarType)>,
}

impl CnvVars {
    pub fn new() -> CnvVars {
        Self::default()
    }

    fn add_var(
        &mut self,
        func_name: &str,
        arg: &Arg,
        builder: &mut VarBuilder,
        ty: CnvVarType,
    ) -> Result<usize, String> {
        match arg {
            Arg::Func(func) => {
                if let Some((source_id, _)) = builder.register_var(func.name, func.args())? {
                    return Ok(builder.store_register((source_id, ty), &mut self.num_vars));
                } else {
                    // if unknown, we issue an error
                    return Err(format!(
                        "Unknown variable/function passed to '{}': {}",
                        func_name, func.name
                    ));
                }
            }
            Arg::Str(s) => {
                return Ok(builder.store_register((s.to_string(), ty), &mut self.fixed_vars));
            }
            #[cfg(feature = "expr")]
            Arg::Expr(_) => unreachable!(),
        };
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
            let target_id = match var {
                CnvVar::Num { expression } => {
                    self.add_var("num", &expression, builder, CnvVarType::Num)?
                }
                // CnvVar::Int { expression } => {
                //     self.add_var("int", &expression, builder, CnvVarType::Int)?
                // }
                CnvVar::Bin {
                    expression,
                    interval,
                } => self.add_var("bin", &expression, builder, CnvVarType::Bin(interval))?,
            };
            return Ok(Some((target_id, out_type)));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        !self.num_vars.is_empty() || !self.fixed_vars.is_empty()
    }

    fn set_record(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        // set fixed-string numbers (only once)
        for (target_id, (text, var_type)) in self.fixed_vars.drain(..) {
            let mut v = Value::default();
            v.set_text(text.as_bytes());
            value_to_symbol!(var_type, v, target_id, symbols, rec)?;
        }
        // dynamic conversion
        for (target_id, (source_id, var_type)) in self.num_vars.iter().cloned() {
            var_type.write_to_symbol(source_id, target_id, symbols, rec)?;
        }
        Ok(())
    }
}
