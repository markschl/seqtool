use crate::config;
use crate::error::CliResult;
use crate::io::{RecordEditor, SeqAttr};
use crate::opt;
use crate::var::*;

pub static USAGE: &str = concat!(
    "
Replaces the contents of sequence IDs, descriptions or sequences.

Usage:
    st set [options][-a <attr>...][-l <list>...] [<input>...]
    st set (-h | --help)
    st set --help-vars

Options:
    -i, --id <expr>     New ID (variables allowed)
    -d, --desc <expr>   New description (variables allowed)
    -s, --seq <expr>    New sequence (variables allowed)
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    let mut replacements = vec![];
    if let Some(string) = args.opt_str("--id") {
        replacements.push((string, SeqAttr::Id));
    }
    if let Some(string) = args.opt_str("--desc") {
        replacements.push((string, SeqAttr::Desc));
    }
    if let Some(string) = args.opt_str("--seq") {
        replacements.push((string, SeqAttr::Seq));
    }

    cfg.writer(|writer, vars| {
        // get String -> VarString
        let replacements: Vec<_> = replacements
            .iter()
            .map(|&(e, attr)| {
                let e = vars.build(|b| varstring::VarString::parse_register(e, b))?;
                Ok((e, attr))
            })
            .collect::<CliResult<_>>()?;

        let mut editor = RecordEditor::new();

        cfg.read(vars, |record, vars| {
            for &(ref expr, attr) in &replacements {
                let val = editor.edit(attr);
                expr.compose(val, vars.symbols(), record);
            }

            writer.write(&editor.rec(&record), vars)?;
            Ok(true)
        })
    })
}
