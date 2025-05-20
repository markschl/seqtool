use std::iter::repeat_n;

use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;
use crate::io::OwnedRecord;

pub const DESC: &str = "\
The sequence IDs must be in the same order in all files;
Fails if the IDs don't match.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Concat' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct ConcatCommand {
    /// Don't check if the IDs of the records from
    /// the different files match
    #[arg(short, long, short)]
    no_id_check: bool,

    /// Add a spacer of <N> characters inbetween the concatenated
    /// sequences.
    #[arg(short, long, short)]
    spacer: Option<usize>,

    /// Character to use as spacer for sequences
    #[arg(short('c'), long, default_value = "N")]
    s_char: char,

    /// Character to use as spacer for qualities.
    /// Defaults to a phred score of 41 (Illumina 1.8+/Phred+33 encoding, which
    /// is the default assumed encoding).
    #[arg(short = 'Q', long, default_value = "J")]
    q_char: char,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: ConcatCommand) -> CliResult<()> {
    let id_check = !args.no_id_check;
    let spacer_n = args.spacer;
    let s_char = args.s_char as u8;
    let q_char = args.q_char as u8;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut record = OwnedRecord::default();
        let num_readers = cfg.num_readers();
        if num_readers == 0 {
            return fail!("Nothing to concatenate!");
        }
        let max_idx = num_readers - 1;

        cfg.read_alongside(false, |i, rec, ctx| {
            let rec_id = rec.id();

            if i == 0 {
                // initialize record
                record.id.clear();
                record.id.extend(rec_id);
                if let Some(d) = rec.desc() {
                    let desc = record.desc.get_or_insert_with(Vec::new);
                    desc.clear();
                    desc.extend(d);
                }
                record.seq.clear();
            } else if id_check && rec_id != record.id.as_slice() {
                return fail!(format!(
                    "ID of record #{} ({}) does not match the ID of the first one ({})",
                    i + 1,
                    String::from_utf8_lossy(rec_id),
                    String::from_utf8_lossy(&record.id)
                ));
            }

            // extend seq
            for s in rec.seq_segments() {
                record.seq.extend(s);
            }

            // handle qual
            if let Some(q) = rec.qual() {
                let qual = record.qual.get_or_insert_with(Vec::new);
                if i == 0 {
                    qual.clear();
                }
                qual.extend(q);
            }

            // spacer
            if let Some(n) = spacer_n {
                if i < max_idx {
                    record.seq.extend(repeat_n(s_char, n));
                    if let Some(q) = record.qual.as_mut() {
                        q.extend(repeat_n(q_char, n));
                    }
                }
            }

            // write at last
            if i == max_idx {
                // handle variables (read_alongside requires this to be done manually)
                ctx.set_record(&record)?;
                format_writer.write(&record, io_writer, ctx)?;
            }
            Ok(true)
        })
    })
}
