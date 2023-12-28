use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::io::{RecordEditor, SeqAttr};
use crate::opt::CommonArgs;
use crate::var::*;

/// Set a new sequence and/or header
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SetCommand {
    /// New ID (variables allowed)
    #[arg(short, long)]
    id: Option<String>,

    /// New description (variables allowed)
    #[arg(short, long)]
    desc: Option<String>,

    /// New sequence (variables allowed)
    #[arg(short, long)]
    seq: Option<String>,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &SetCommand) -> CliResult<()> {
    let mut replacements = vec![];
    if let Some(string) = args.id.as_ref() {
        replacements.push((string, SeqAttr::Id));
    }
    if let Some(string) = args.desc.as_ref() {
        replacements.push((string, SeqAttr::Desc));
    }
    if let Some(string) = args.seq.as_ref() {
        replacements.push((string, SeqAttr::Seq));
    }

    cfg.writer(|writer, io_writer, vars| {
        // get String -> VarString
        let replacements: Vec<_> = replacements
            .iter()
            .map(|&(e, attr)| {
                let (e, _) = vars.build(|b| varstring::VarString::parse_register(e, b))?;
                Ok((e, attr))
            })
            .collect::<CliResult<_>>()?;

        let mut editor = RecordEditor::new();

        cfg.read(vars, |record, vars| {
            for &(ref expr, attr) in &replacements {
                let val = editor.edit(attr);
                expr.compose(val, vars.symbols(), record)?;
            }

            writer.write(&editor.rec(&record), io_writer, vars)?;
            Ok(true)
        })
    })
}
