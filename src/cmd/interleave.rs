use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;

pub const DESC: &str = "\
The records are returned in the same order as in the input files.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Interleave' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct InterleaveCommand {
    /// Don't check if the IDs of the files match
    #[arg(short, long)]
    no_id_check: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: &InterleaveCommand) -> CliResult<()> {
    let id_check = !args.no_id_check;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut id = vec![];

        cfg.read_alongside(|i, rec, ctx| {
            if id_check {
                let rec_id = rec.id();
                if i == 0 {
                    id.clear();
                    id.extend(rec_id);
                } else if rec_id != id.as_slice() {
                    return fail!(format!(
                        "ID of record #{} ({}) does not match the ID of the first one ({})",
                        i + 1,
                        String::from_utf8_lossy(rec_id),
                        String::from_utf8_lossy(&id)
                    ));
                }
            }
            // handle variables (read_alongside requires this to be done manually)
            ctx.set_record(&rec)?;

            format_writer.write(rec, io_writer, ctx)
        })
    })
}
