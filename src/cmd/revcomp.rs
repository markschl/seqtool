use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::complement::reverse_complement;
use crate::io::SeqQualRecord;

use crate::helpers::seqtype::{SeqType, SeqtypeHelper};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
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

pub fn run(mut cfg: Config, args: &RevcompCommand) -> CliResult<()> {
    let num_threads = args.threads;

    let mut format_writer = cfg.get_format_writer()?;
    let typehint = cfg.get_seqtype();
    let mut final_seqtype = None;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel_init(
            num_threads - 1,
            || Ok(SeqtypeHelper::new(typehint)),
            Default::default,
            |record, out: &mut Box<RevCompRecord>, st_helper| {
                if out.seqtype.is_none() {
                    out.seqtype = Some(st_helper.get_or_guess(record)?);
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
