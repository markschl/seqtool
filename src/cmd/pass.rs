use clap::Parser;

use crate::cli::{CommonArgs, Report};
use crate::config::Config;
use crate::error::CliResult;

#[derive(Parser, Clone, Debug)]
pub struct PassCommand {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, _args: PassCommand) -> CliResult<Option<Box<dyn Report>>> {
    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read(|record, ctx| {
            format_writer.write(&record, io_writer, ctx)?;
            Ok(true)
        })
    })
    .map(|r| Some(Report::to_box(r)))
}
