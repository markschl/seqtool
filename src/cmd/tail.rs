use std::cmp::max;

use clap::{value_parser, Parser};

use crate::cli::{CommonArgs, Report, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;

pub const DESC: &str = "\
This only works for files (not STDIN), since records are counted in a first
step, and only returned after reading a second time.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Tail' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct TailCommand {
    /// Number of sequences to return
    #[arg(short, long, value_name = "N", default_value_t = 10, value_parser = value_parser!(u64).range(1..))]
    num_seqs: u64,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: TailCommand) -> CliResult<Option<Box<dyn Report>>> {
    let n_select = args.num_seqs;

    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        // first count the sequences
        // TODO: use .fai files once supported?
        let mut n = 0;

        cfg.read(|_, _| {
            n += 1;
            Ok(true)
        })?;

        let mut i = 0;
        let select_from = max(n, n_select) - n_select;

        cfg.read(|record, ctx| {
            i += 1;
            if i > select_from {
                format_writer.write(&record, io_writer, ctx)?;
            }
            Ok(true)
        })
    })
    .map(|r| Some(Report::to_box(r)))
}
