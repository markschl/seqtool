use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::rng::Range;

pub const DESC: &str = "\
The range is specified as `start..end`, whereby start and end
are the sequence numbers (starting from 1). Open ranges are
possible, in the form `start..` or `..end`.

The following is equivalent with the
'head' and 'tail' commands:
 `st slice ..10 input.fasta`
 `st slice '-10..' input.fasta`

The 'slice' command does not extract subsequences; see the
'trim' command for that.";
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Slice' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct SliceCommand {
    /// Range in form 'start..end' or '..end' or 'start..'
    #[arg(value_name = "FROM..TO")]
    range: Range,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: &SliceCommand) -> CliResult<()> {
    let range = args.range;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        // convert from 1-based to 0-based coordinates
        let mut start = range.start.unwrap_or(1);
        if start == 0 {
            return fail!("Select ranges are 1-based, zero is not a valid start value");
        }
        start -= 1;
        let end = range.end;

        let mut i = 0;

        cfg.read(|record, ctx| {
            // if a start value was specified, skip records
            // was thinking about using Itertools::dropping(), but have to check for errors...
            if i >= start {
                if let Some(e) = end {
                    if i >= e {
                        return Ok(false);
                    }
                }
                format_writer.write(&record, io_writer, ctx)?;
            }
            i += 1;
            Ok(true)
        })
    })
}
