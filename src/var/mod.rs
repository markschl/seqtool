use std::fs::File;

use crate::error::CliResult;
use crate::io::input::InFormat;
use crate::io::{QualFormat, SeqAttr};
use crate::helpers::util::parse_delimiter;

pub use self::var::*;

pub mod attr;
pub mod modules;
pub mod symbols;
mod var;
pub mod varstring;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VarOpts<'a> {
    // metadata
    pub lists: Vec<&'a str>,
    pub list_delim: &'a str,
    pub has_header: bool,
    pub unordered: bool,
    pub id_col: usize,
    pub allow_missing: bool,
    // attributes
    pub attr_opts: AttrOpts,
    // expressions
    pub expr_init: Option<&'a str>,
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
    let help_mod: &[Box<dyn var::VarHelp>] = &[
        Box::new(modules::builtins::BuiltinHelp),
        Box::new(modules::stats::StatHelp),
        Box::new(modules::attr::AttrHelp),
        Box::new(modules::list::ListHelp),
        Box::new(modules::expr::ExprHelp),
    ];
    help_mod
        .iter()
        .map(|m| m.format())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn get_vars(o: &VarOpts, informat: &InFormat) -> CliResult<Vars> {
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
    let qual_converter = match *informat {
        InFormat::Fastq { format } => format,
        InFormat::FaQual { .. } => QualFormat::Phred,
        _ => QualFormat::Sanger,
    }
    .get_converter();

    let mut vars = Vars::new(delim, value_delim, append_attr, qual_converter);

    // lists
    let list_delim = parse_delimiter(o.list_delim)?;
    for (i, &list) in o.lists.iter().enumerate() {
        let csv_file = File::open(list).map_err(|e| format!("Error opening '{}': {}", list, e))?;
        if o.unordered {
            let finder = modules::list::Unordered::new();
            vars.add_module(
                modules::list::ListVars::new(i + 1, o.lists.len(), csv_file, finder, list_delim)
                    .id_col(o.id_col)
                    .has_header(o.has_header)
                    .allow_missing(o.allow_missing),
            );
        } else {
            let finder = modules::list::SyncIds;
            vars.add_module(
                modules::list::ListVars::new(i + 1, o.lists.len(), csv_file, finder, list_delim)
                    .id_col(o.id_col)
                    .has_header(o.has_header)
                    .allow_missing(o.allow_missing),
            );
        }
    }

    // other modules
    vars.add_module(modules::builtins::BuiltinVars::new());

    vars.add_module(modules::stats::StatVars::new());

    // TODO: allow_missing may not be used at all, a separate option may not make sense
    vars.add_module(modules::attr::AttrVars::new());

    vars.add_module(modules::expr::ExprVars::new(o.expr_init)?);

    Ok(vars)
}
