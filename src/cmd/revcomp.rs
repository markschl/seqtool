use std::sync::OnceLock;

use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{
    complement::reverse_complement,
    seqtype::{SeqType, SeqtypeHelper},
};
use crate::io::SeqQualRecord;

pub const DESC: &str = "\
The sequence type is automatically detected based on the first record,
unless the `--seqtype` option is used.

*Note*: Unknown letters are not reversed, but left unchanged.

If quality scores are present, their order is just reversed";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Revcomp' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct RevcompCommand {
    /// Number of threads to use
    #[arg(short, long, default_value_t = 1)]
    threads: u32,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Default, Clone, Debug)]
struct RevCompRecord {
    seq: Vec<u8>,
    qual: Option<Vec<u8>>,
    seqtype: Option<SeqType>,
}

// TODO: wait for https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get_or_try_init stabilization
static SEQTYPE: OnceLock<Result<SeqType, String>> = OnceLock::new();

pub fn run(mut cfg: Config, args: RevcompCommand) -> CliResult<()> {
    let num_threads = args.threads;

    let mut format_writer = cfg.get_format_writer()?;
    let typehint = cfg.input_config[0].format.seqtype;
    let mut final_seqtype = None;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel_init(
            num_threads - 1,
            Default::default,
            |record, out: &mut Box<RevCompRecord>| {
                if out.seqtype.is_none() {
                    let seqtype = SEQTYPE.get_or_init(|| {
                        SeqtypeHelper::new(typehint).get_or_guess(record)
                    }).clone()?;
                    out.seqtype = Some(seqtype);
                }
                reverse_complement(record.seq_segments(), &mut out.seq, out.seqtype.unwrap())?;
                if let Some(q) = record.qual() {
                    let qual = out.qual.get_or_insert_with(|| Vec::with_capacity(q.len()));
                    qual.clear();
                    qual.extend(q.iter().rev());
                }
                Ok(())
            },
            |record, revcomp_record, ctx| {
                if final_seqtype.is_none() {
                    final_seqtype = revcomp_record.seqtype;
                } else if revcomp_record.seqtype != final_seqtype {
                    // fail if there is a mismatch in sequence types guessed in different threads
                    return fail!("Could not reliably guess the sequence type. Please specify with `--seqtype`");
                }
                let rc_rec = SeqQualRecord::new(
                    &record,
                    &revcomp_record.seq,
                    revcomp_record.qual.as_deref(),
                );
                format_writer.write(&rc_rec, io_writer, ctx)?;
                Ok(true)
            },
        )
    })?;
    Ok(())
}
