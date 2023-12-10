use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::rng::Range;
use crate::opt::CommonArgs;

/// Returns a slice of the sequences within a defined range.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SliceCommand {
    /// Range in form 'start..end' or '..end' or 'start..'
    #[arg(value_name = "FROM..TO")]
    range: Range,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &SliceCommand) -> CliResult<()> {
    let range = args.range.clone();

    cfg.writer(|writer, vars| {
        // convert from 1-based to 0-based coordinates
        let mut start = range.start.unwrap_or(1);
        if start == 0 {
            return fail!("Select ranges are 1-based, zero is not a valid start value");
        }
        start -= 1;
        let end = range.end;

        let mut i = 0;

        cfg.read(vars, |record, vars| {
            // if a start value was specified, skip records
            // was thinking about using Itertools::dropping(), but have to check for errors...
            if i >= start {
                if let Some(e) = end {
                    if i >= e {
                        return Ok(false);
                    }
                }
                writer.write(&record, vars)?;
            }
            i += 1;
            Ok(true)
        })
    })
}
