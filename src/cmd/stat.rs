use clap::Parser;

use crate::error::CliResult;
use crate::opt::CommonArgs;
use crate::{config::Config, io::FormatVariant};

use super::pass::{self, PassCommand};

/// Returns per sequence statistics as tab delimited list. All variables
/// (seqlen, exp_err, charcount(...), etc.) can be used (see `st stat --help-vars`).
/// The command is equivalent to `st pass --to-tsv 'id,var1,var2,...' input`
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct StatCommand {
    /// Comma delimited list of statistics variables.
    #[arg(value_name = "VAR")]
    vars: String,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(_cfg: Config, args: &StatCommand) -> CliResult<()> {
    let mut cmd = PassCommand {
        common: args.common.clone(),
    };
    cmd.common.output.to_tsv = Some("id,".to_string() + &args.vars);
    if let Some(info) = cmd.common.output.to.as_mut() {
        info.format = FormatVariant::Tsv;
    }
    pass::run(Config::new(&cmd.common)?, &cmd)
}
