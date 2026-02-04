use clap::Parser;

use crate::cli::{CommonArgs, Report, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::rng::Range;

pub const DESC: &str = "\
The range is specified as `start:end`, whereby start and end
are the sequence numbers (starting from 1). Open ranges are
possible, in the form `start:` or `:end`.

The following is equivalent with `st head input.fasta`:
`st slice ':10' input.fasta`

The following is equivalent with `st tail input.fasta`:
 `st slice '-10:' input.fasta`

The 'slice' command does not extract subsequences; see the
'trim' command for that.";
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Slice' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct SliceCommand {
    /// Range in form 'start:end' or ':end' or 'start:'
    #[arg(value_name = "FROM:TO")]
    range: Range,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: SliceCommand) -> CliResult<Option<Box<dyn Report>>> {
    let range = args.range;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        // convert from 1-based to 0-based coordinates
        let start = range.start.unwrap_or(1);
        if start == 0 {
            return fail!("Slice ranges are 1-based, zero is not a valid start value");
        }
        if start < 0 || range.end.map(|e| e < 0).unwrap_or(false) {
            return fail!("Slice ranges cannot be negative");
        }
        let start = start as u64;
        let end = range.end.map(|e| e as u64);

        cfg.read(|record, ctx| {
            // if a start value was specified, skip records
            // was thinking about using Itertools::dropping(), but have to check for errors...
            if ctx.n_records >= start {
                if let Some(e) = end {
                    if ctx.n_records > e {
                        return Ok(false);
                    }
                }
                format_writer.write(&record, io_writer, ctx)?;
            }
            Ok(true)
        })
    })
    .map(|r| Some(Report::to_box(r)))
}
