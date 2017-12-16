use std::fs::File;

use error::CliResult;
use lib::util::parse_delimiter;
use io::Attribute;

pub use self::var::*;

mod var;
pub mod modules;
pub mod prop;
pub mod symbols;
pub mod varstring;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VarOpts<'a> {
    pub lists: Vec<&'a str>,
    pub list_delim: &'a str,
    pub has_header: bool,
    pub unordered: bool,
    pub id_col: usize,
    pub prop_opts: PropOpts,
    pub allow_missing: bool,
    // Used to remember that the variable help page has to be returned
    pub var_help: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct PropOpts {
    pub delim: String,
    pub value_delim: String,
}

impl Default for PropOpts {
    fn default() -> Self {
        PropOpts {
            delim: " ".to_string(),
            value_delim: "=".to_string(),
        }
    }
}

pub fn var_help() -> String {
    let help_mod: &[Box<var::VarHelp>] = &[
        Box::new(modules::builtins::BuiltinHelp),
        Box::new(modules::stats::StatHelp),
        Box::new(modules::prop::PropHelp),
        Box::new(modules::list::ListHelp),
        Box::new(modules::expr::ExprHelp),
    ];
    help_mod
        .into_iter()
        .map(|m| m.format())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn get_vars<'a>(o: &VarOpts) -> CliResult<Vars<'a>> {
    // Vars instance
    let delim = parse_delimiter(&o.prop_opts.delim)?;
    let value_delim = parse_delimiter(&o.prop_opts.value_delim)?;
    let append_attr = if delim == b' ' {
        Attribute::Desc
    } else {
        Attribute::Id
    };
    let mut vars = Vars::new(delim, value_delim, append_attr);

    // lists
    let list_delim = parse_delimiter(o.list_delim)?;
    for (i, list) in o.lists.iter().enumerate() {
        let csv_file = File::open(list)?;
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

    vars.add_module(modules::prop::PropVars::new(o.allow_missing));

    vars.add_module(modules::expr::ExprVars::new()?);

    Ok(vars)
}
