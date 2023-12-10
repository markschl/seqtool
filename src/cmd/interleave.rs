use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;

/// Interleaves records of all files in the input. The records will returned in
/// the same order as the files.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct InterleaveCommand {
    /// Don't check if the IDs of the files match
    #[arg(short, long)]
    no_id_check: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &InterleaveCommand) -> CliResult<()> {
    let id_check = !args.no_id_check;

    cfg.writer(|writer, vars| {
        let mut id = vec![];

        cfg.read_alongside(|i, rec| {
            if id_check {
                let rec_id = rec.id_bytes();
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
            writer.write(rec, vars)
        })
    })
}
