use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::io::{RecordEditor, SeqAttr};
use crate::var::varstring::VarString;

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

pub fn run(mut cfg: Config, args: &SetCommand) -> CliResult<()> {
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

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        // get String -> VarString
        let replacements: Vec<_> = replacements
            .iter()
            .map(|&(e, attr)| {
                let (e, _) = cfg.build_vars(|b| VarString::parse_register(e, b, false))?;
                Ok((e, attr))
            })
            .collect::<CliResult<_>>()?;

        let mut editor = RecordEditor::new();

        cfg.read(|record, ctx| {
            for &(ref expr, attr) in &replacements {
                let val = editor.edit(attr);
                expr.compose(val, &ctx.symbols, record)?;
            }

            format_writer.write(&editor.rec(&record), io_writer, ctx)?;
            Ok(true)
        })
    })
}
