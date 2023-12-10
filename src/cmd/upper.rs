use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::io::SeqQualRecord;
use crate::opt::CommonArgs;

/// Converts all characters in the sequence to uppercase.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct UpperCommand {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, _args: &UpperCommand) -> CliResult<()> {
    cfg.writer(|writer, vars| {
        let mut seq = vec![];
        cfg.read(vars, |record, vars| {
            seq.clear();
            for s in record.seq_segments() {
                seq.extend(s.iter().cloned().map(|ref mut b| {
                    b.make_ascii_uppercase();
                    *b
                }));
            }
            let ucase_rec = SeqQualRecord::new(&record, &seq, None);
            writer.write(&ucase_rec, vars)?;
            Ok(true)
        })
    })
}
