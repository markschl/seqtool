use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct PassCommand {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, _args: &PassCommand) -> CliResult<()> {
    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read(|record, ctx| {
            format_writer.write(&record, io_writer, ctx)?;
            Ok(true)
        })
    })
}
