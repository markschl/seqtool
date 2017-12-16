use error::CliResult;
use opt;
use io::{Attribute, RecordEditor};
use var::*;

use cfg;

pub static USAGE: &'static str = concat!("
Replaces the contents of sequence IDs, descriptions or sequences.

Usage:
    seqtool set [options][-p <prop>...][-l <list>...] [<input>...]
    seqtool set (-h | --help)
    seqtool set --help-vars

Options:
    -i, --id <expr>     New ID (variables allowed)
    -d, --desc <expr>   New description (variables allowed)
    -s, --seq <expr>    New sequence (variables allowed)

", common_opts!());

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let mut replacements = vec![];
    if let Some(string) = args.opt_str("--id") {
        replacements.push((string, Attribute::Id));
    }
    if let Some(string) = args.opt_str("--desc") {
        replacements.push((string, Attribute::Desc));
    }
    if let Some(string) = args.opt_str("--seq") {
        replacements.push((string, Attribute::Seq));
    }

    cfg.writer(|writer, mut vars| {
        // get String -> VarString
        let replacements: Vec<_> = replacements
            .iter()
            .map(|&(e, attr)| {
                let e = vars.build(|b| varstring::VarString::parse_register(e, b))?;
                Ok((e, attr))
            })
            .collect::<CliResult<_>>()?;

        let mut editor = RecordEditor::new();

        cfg.read_sequential_var(&mut vars, |record, vars| {
            for &(ref expr, attr) in &replacements {
                let val = editor.edit(attr);
                expr.compose(val, vars.symbols())
            }

            writer.write(&editor.rec(&record), vars)?;
            Ok(true)
        })
    })
}
