use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;

/// No processing done, useful for converting and attribute setting
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct PassCommand {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, _args: &PassCommand) -> CliResult<()> {
    cfg.writer(|writer, vars| {
        cfg.read(vars, |record, vars| {
            writer.write(&record, vars)?;
            Ok(true)
        })
    })
}
