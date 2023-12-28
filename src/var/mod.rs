use std::fs::File;

use crate::error::CliResult;
use crate::io::input::InFormat;
use crate::io::{QualFormat, SeqAttr};

pub use self::var::*;

pub mod attr;
pub mod modules;
pub mod symbols;
mod var;
pub mod varstring;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VarOpts {
    // metadata
    pub lists: Vec<String>,
    pub list_delim: char,
    pub has_header: bool,
    pub unordered: bool,
    pub id_col: u32,
    pub allow_missing: bool,
    // attributes
    pub attr_opts: AttrOpts,
    // expressions
    pub expr_init: Option<String>,
    // Used to remember that the variable help page has to be returned
    pub var_help: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct AttrOpts {
    pub delim: char,
    pub value_delim: char,
}

impl Default for AttrOpts {
    fn default() -> Self {
        AttrOpts {
            delim: ' ',
            value_delim: '=',
        }
    }
}

pub fn get_vars(
    o: &VarOpts,
    informat: &InFormat,
    custom_mod: Option<Box<dyn VarProvider>>,
) -> CliResult<Vars> {
    // Vars instance
    let append_attr = if o.attr_opts.delim == ' ' {
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

    let mut vars = Vars::new(
        o.attr_opts.delim as u8,
        o.attr_opts.value_delim as u8,
        append_attr,
        qual_converter,
        o.var_help,
    );

    // the custom module needs to be inserted early
    // at least before the expression module
    if let Some(m) = custom_mod {
        vars.add_module(m);
    }

    // lists
    for (i, list) in o.lists.iter().enumerate() {
        let csv_file = File::open(list).map_err(|e| format!("Error opening '{}': {}", list, e))?;
        if o.unordered {
            let finder = modules::list::Unordered::new();
            vars.add_module(Box::new(
                modules::list::ListVars::new(
                    i + 1,
                    o.lists.len(),
                    csv_file,
                    finder,
                    o.list_delim as u8,
                )
                .id_col(o.id_col)
                .has_header(o.has_header)
                .allow_missing(o.allow_missing),
            ));
        } else {
            let finder = modules::list::SyncIds;
            vars.add_module(Box::new(
                modules::list::ListVars::new(
                    i + 1,
                    o.lists.len(),
                    csv_file,
                    finder,
                    o.list_delim as u8,
                )
                .id_col(o.id_col)
                .has_header(o.has_header)
                .allow_missing(o.allow_missing),
            ));
        }
    }

    // other modules
    vars.add_module(Box::new(modules::builtins::BuiltinVars::new()));

    vars.add_module(Box::new(modules::stats::StatVars::new()));

    vars.add_module(Box::new(modules::attr::AttrVars::new()));

    #[cfg(feature = "expr")]
    vars.add_module(Box::new(modules::expr::ExprVars::new(
        o.expr_init.as_deref(),
    )?));

    Ok(vars)
}
