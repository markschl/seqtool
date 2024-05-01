//! Types and functions providing/handling variable/function usage information

use itertools::Itertools;

use crate::VarType;

#[cold]
pub(crate) fn usage_list(info: &FuncUsage) -> Vec<String> {
    let n_args = info.args.len();
    let n_required = info
        .args
        .iter()
        .position(|arg| arg.default_value.is_some())
        .unwrap_or(info.args.len());
    let mut out = Vec::with_capacity(1 + n_args - n_required);
    if n_required == 0 {
        out.push(info.name.to_string());
    }
    if n_args > n_required {
        for i in n_required.clamp(1, n_args)..n_args+1 {
            out.push(format!("{}({})", info.name, info.args[..i].iter().map(|u| u.name).join(", ")));
        }
    }
    out
}

#[derive(Debug)]
pub struct FuncUsage {
    pub name: &'static str,
    // multiple argument collections possible
    // (different usage patterns)
    pub args: &'static [ArgUsage],
    pub description: &'static str,
    pub output_type: Option<VarType>,
    pub hidden: bool,
}

#[derive(Debug)]
pub struct ArgUsage {
    pub name: &'static str,
    // the default value is always specified as &str in the usage string (for the help page), even
    // though from_func() will parse it further
    pub default_value: Option<&'static str>,
}
