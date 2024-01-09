use clap::Parser;

use crate::cli::CommonArgs;
use crate::error::CliResult;
use crate::{config::Config, io::FormatVariant};

use super::pass::{self, PassCommand};

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
