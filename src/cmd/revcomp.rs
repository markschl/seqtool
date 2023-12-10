use std::ops::DerefMut;

use bio::alphabets::dna::complement;
use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::io::SeqQualRecord;
use crate::opt::CommonArgs;

/// Reverse complements DNA sequences. If quality scores are present,
/// their order is just reversed.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct RevcompCommand {
    /// Number of threads to use
    #[arg(short, long, default_value_t = 1)]
    threads: u32,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &RevcompCommand) -> CliResult<()> {
    let num_threads = args.threads;

    cfg.writer(|writer, vars| {
        cfg.parallel_var::<_, _, Box<(Vec<u8>, Vec<u8>, bool)>>(
            vars,
            num_threads - 1,
            |record, data| {
                let (ref mut seq, ref mut qual, ref mut has_qual) = *data.deref_mut();
                seq.clear();
                for s in record.seq_segments().rev() {
                    seq.extend(s.iter().rev().cloned().map(complement));
                }
                if let Some(q) = record.qual() {
                    qual.clear();
                    qual.extend(q.iter().rev());
                    *has_qual = true;
                } else {
                    *has_qual = false;
                }
                Ok(())
            },
            |record, data, vars| {
                let (ref mut seq, ref mut qual, has_qual) = *data.deref_mut();
                let q = if has_qual {
                    Some(qual.as_slice())
                } else {
                    None
                };
                let rc_rec = SeqQualRecord::new(&record, seq, q);
                writer.write(&rc_rec, vars)?;
                Ok(true)
            },
        )
    })?;
    Ok(())
}
