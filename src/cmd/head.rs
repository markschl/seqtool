use clap::{value_parser, Parser};

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;

/// Returns the first sequences of the input.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct HeadCommand {
    /// Number of sequences to return
    #[arg(short, long, value_name = "N", default_value_t = 10, value_parser = value_parser!(u64).range(1..))]
    num_seqs: u64,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: &HeadCommand) -> CliResult<()> {
    let n = args.num_seqs;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut i = 0;

        cfg.read(|record, ctx| {
            if i >= n {
                return Ok(false);
            }
            format_writer.write(&record, io_writer, ctx)?;
            i += 1;
            Ok(true)
        })
    })
}
