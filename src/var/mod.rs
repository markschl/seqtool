use std::fs::File;

use error::CliResult;
use lib::util::parse_delimiter;
use io::input::InFormat;
use io::{SeqAttr, QualFormat};

pub use self::var::*;

#[cfg(not(feature = "exprtk"))]
use self::modules::expr as expr_module;
#[cfg(feature = "exprtk")]
use self::modules::expr_exprtk as expr_module;

mod var;
pub mod modules;
pub mod attr;
pub mod symbols;
pub mod varstring;



#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VarOpts<'a> {
    pub lists: Vec<&'a str>,
    pub list_delim: &'a str,
    pub has_header: bool,
    pub unordered: bool,
    pub id_col: usize,
    pub attr_opts: AttrOpts,
    pub allow_missing: bool,
    // Used to remember that the variable help page has to be returned
    pub var_help: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct AttrOpts {
    pub delim: String,
    pub value_delim: String,
}

impl Default for AttrOpts {
    fn default() -> Self {
        AttrOpts {
            delim: " ".to_string(),
            value_delim: "=".to_string(),
        }
    }
}

pub fn var_help() -> String {
    let help_mod: &[Box<var::VarHelp>] = &[
        Box::new(modules::builtins::BuiltinHelp),
        Box::new(modules::stats::StatHelp),
        Box::new(modules::attr::AttrHelp),
        Box::new(modules::list::ListHelp),
        Box::new(expr_module::ExprHelp),
    ];
    help_mod
        .into_iter()
        .map(|m| m.format())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn get_vars<'a>(o: &VarOpts, informat: &InFormat) -> CliResult<Vars<'a>> {
    // Vars instance
    let delim = parse_delimiter(&o.attr_opts.delim)?;
    let value_delim = parse_delimiter(&o.attr_opts.value_delim)?;
    let append_attr = if delim == b' ' {
        SeqAttr::Desc
    } else {
        SeqAttr::Id
    };
    // quality converter is not related to variables,
    // therefore stored in InFormat
    let qual_converter =
        match *informat {
            InFormat::FASTQ { format } => format,
            _ => QualFormat::Sanger
        }
        .get_converter();

    let mut vars = Vars::new(delim, value_delim, append_attr, qual_converter);

    // lists
    let list_delim = parse_delimiter(o.list_delim)?;
    for (i, &list) in o.lists.iter().enumerate() {
        let csv_file = File::open(list)
            .map_err(|e| format!("Error opening '{}': {}", list, e))?;
        if o.unordered {
            let finder = modules::list::Unordered::new();
            vars.add_module(modules::list::ListVars::new(
                i + 1, csv_file, finder,
                o.id_col, list_delim, o.has_header, o.allow_missing,
            ));
        } else {
            let finder = modules::list::SyncIds;
            vars.add_module(modules::list::ListVars::new(
                i + 1, csv_file, finder,
                o.id_col, list_delim, o.has_header, o.allow_missing,
            ));
        }
    }

    // other modules
    vars.add_module(modules::builtins::BuiltinVars::new());

    vars.add_module(modules::stats::StatVars::new());

    vars.add_module(modules::attr::AttrVars::new(o.allow_missing));

    vars.add_module(expr_module::ExprVars::new()?);

    Ok(vars)
}
