use clap::{value_parser, Parser};

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;

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

pub fn run(cfg: Config, args: &HeadCommand) -> CliResult<()> {
    let n = args.num_seqs;

    cfg.writer(|writer, vars| {
        let mut i = 0;

        cfg.read(vars, |record, vars| {
            if i >= n {
                return Ok(false);
            }
            writer.write(&record, vars)?;
            i += 1;
            Ok(true)
        })
    })
}
