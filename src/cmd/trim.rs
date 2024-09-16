use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{rng::Range, var_range::VarRanges};
use crate::io::{Record, SeqQualRecord};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Trim' command options")]
pub struct TrimCommand {
    /// Range(s) in the form 'start:end' or 'start:' or ':end',
    /// Multiple ranges can be supplied as comma-delimited list:
    /// 'start:end,start2:end2', etc.
    /// The start/end positions can be defined by variables/functions
    /// (start_var:end_var), or variables/functions may return
    /// the whole range (e.g. stored as header attribute 'attr(range)'),
    /// or even a list of ranges (e.g. 'attr(range_list)').
    /// *Note* that with the FASTA format, multiple trim ranges
    /// must be in order (from left to right) and cannot overlap.
    #[arg(allow_hyphen_values = true)]
    ranges: String,

    /// Exclusive trim range: excludes start and end positions
    /// from the output sequence.
    /// In the case of unbounded ranges (`start:` or `:end`), the range still
    /// extends to the complete end or the start of the sequence.
    #[arg(short, long)]
    exclusive: bool,

    /// Interpret range as 0-based, with the end not included.
    #[arg(short('0'), long)]
    zero_based: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: &TrimCommand) -> CliResult<()> {
    let ranges = &args.ranges;
    let rng0 = args.zero_based;
    let exclusive = args.exclusive;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut out_seq = Vec::new();
        let mut out_qual = Vec::new();

        let mut ranges = cfg.build_vars(|b| VarRanges::from_str(ranges, b))?;
        let mut num_buf = Vec::new();

        cfg.read(|record, ctx| {
            let ranges = ranges.resolve(&ctx.symbols, record, &mut num_buf)?;

            let rec = trim(
                &record,
                ranges,
                &mut out_seq,
                &mut out_qual,
                rng0,
                exclusive,
            )?;

            format_writer.write(&rec, io_writer, ctx)?;
            Ok(true)
        })
    })
}

fn trim<'r>(
    record: &'r dyn Record,
    ranges: &[Range],
    out_seq: &'r mut Vec<u8>,
    out_qual: &'r mut Vec<u8>,
    rng0: bool,
    exclusive: bool,
) -> CliResult<SeqQualRecord<'r, &'r dyn Record>> {
    // TODO: only needed with negative bounds -> maybe check if there are neg. bounds or not
    // and only calculate sequence length if needed (adjust Range::obtain()) as well
    let seqlen = record.seq_len();
    out_seq.clear();

    if let Some(qual) = record.qual() {
        // We assume *no* multiline sequence (FASTQ), which allows for simpler code
        // TODO: may change in future!
        let seq = record.raw_seq();
        out_qual.clear();
        for rng in ranges {
            let (start, end) = rng.adjust(rng0, exclusive)?.obtain(seqlen);
            out_seq.extend_from_slice(&seq[start..end]);
            out_qual.extend_from_slice(&qual[start..end]);
        }
        Ok(SeqQualRecord::new(record, out_seq, Some(out_qual)))
    } else {
        // FASTA format
        let mut seq_iter = record.seq_segments();
        let mut seq = seq_iter.next();
        let mut offset = 0;
        'outer: for rng in ranges {
            let (mut start, mut end) = rng.adjust(rng0, exclusive)?.obtain(seqlen);
            if start < offset {
                return fail!(
                    "Unsorted/overlapping trim ranges encountered. This is only \
                    possible if FASTA lines are long enough. \
                    To fix this, either supply single-line FASTA (no --wrap) or \
                    make sure that trim ranges are in order and/or don't overlap \
                    to an extent that this error occurs."
                );
            }
            start -= offset;
            end -= offset;
            loop {
                if let Some(segment) = seq {
                    if start < segment.len() {
                        if end <= segment.len() {
                            // requested fragment is fully contained in segment
                            // -> continue to next range (if any)
                            out_seq.extend_from_slice(&segment[start..end]);
                            break;
                        }
                        // requested fragment is larger than sequence segment
                        // -> obtain next segment and continue
                        out_seq.extend_from_slice(&segment[start..]);
                        start = 0;
                    } else {
                        start -= segment.len();
                    }
                    offset += segment.len();
                    end -= segment.len();
                    seq = seq_iter.next();
                } else {
                    // last sequence segment visited -> done
                    // (not all ranges may be "consumed" yet)
                    break 'outer;
                }
            }
        }
        Ok(SeqQualRecord::new(record, out_seq, None))
    }
}
