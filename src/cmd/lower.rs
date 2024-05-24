use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::io::SeqQualRecord;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Lower' command options")]
pub struct LowerCommand {
    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, _args: &LowerCommand) -> CliResult<()> {
    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut seq = vec![];
        cfg.read(|record, ctx| {
            seq.clear();
            for s in record.seq_segments() {
                seq.extend(s.iter().cloned().map(|ref mut b| {
                    b.make_ascii_lowercase();
                    *b
                }));
            }
            let ucase_rec = SeqQualRecord::new(&record, &seq, None);
            format_writer.write(&ucase_rec, io_writer, ctx)?;
            Ok(true)
        })
    })
}
