use std::cmp::max;

use clap::{value_parser, Parser};

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;

/// Returns the last sequences of the input.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct TailCommand {
    /// Number of sequences to return
    #[arg(short, long, value_name = "N", default_value_t = 10, value_parser = value_parser!(u64).range(1..))]
    num_seqs: u64,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &TailCommand) -> CliResult<()> {
    let n_select = args.num_seqs;

    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    cfg.writer(|writer, vars| {
        // first count the sequences
        // TODO: maybe support .fai files and use them?
        let mut n = 0;

        cfg.read_simple(|_| {
            n += 1;
            Ok(true)
        })?;

        let mut i = 0;
        let select_from = max(n, n_select) - n_select;

        cfg.read(vars, |record, vars| {
            i += 1;
            if i > select_from {
                writer.write(&record, vars)?;
            }
            Ok(true)
        })
    })
}
