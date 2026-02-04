use clap::Parser;

use crate::cli::{CommonArgs, Report, WORDY_HELP};
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::var_range::VarRanges;
use crate::io::SeqQualRecord;

pub const DESC: &str = "\
Masks the sequence within a given range or comma delimited list of ranges
by converting to lowercase (soft mask) or replacing with a character (hard
masking). Reverting soft masking is also possible.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Mask' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct MaskCommand {
    /// Range in the form 'start:end' or 'start:' or ':end',
    /// The range start/end may be defined by varialbes/functions,
    /// or the varialbe/function may contain a whole range.
    ranges: String,

    /// Do hard masking instead of soft masking, replacing
    /// everything in the range(s) with the given character
    #[arg(long, value_name = "CHAR")]
    hard: Option<char>,

    /// Unmask (convert to uppercase instead of lowercase)
    #[arg(long)]
    unmask: bool,

    /// Exclusive range: excludes start and end positions
    /// from the masked sequence.
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

pub fn run(mut cfg: Config, args: MaskCommand) -> CliResult<Option<Box<dyn Report>>> {
    let ranges = &args.ranges;
    let hard_mask = args.hard;
    let rng0 = args.zero_based;
    let exclusive = args.exclusive;
    let unmask = args.unmask;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let mut ranges = cfg.build_vars(|b| VarRanges::from_str(ranges, b))?;
        let mut seq = Vec::new();
        let mut num_buf = Vec::new();

        cfg.read(|record, ctx| {
            // obtain full sequence
            seq.clear();
            let mut seqlen = 0;
            for s in record.seq_segments() {
                seq.extend_from_slice(s);
                seqlen += s.len();
            }

            let calc_ranges = ranges.resolve(ctx.symbols(), record, &mut num_buf)?;

            if let Some(h) = hard_mask {
                for rng in calc_ranges {
                    let (start, end) = rng.adjust(rng0, exclusive)?.resolve(seqlen);
                    for c in &mut seq[start..end] {
                        *c = h as u8;
                    }
                }
            } else {
                for rng in calc_ranges {
                    let (start, end) = rng.adjust(rng0, exclusive)?.resolve(seqlen);
                    for c in &mut seq[start..end] {
                        if unmask {
                            c.make_ascii_uppercase()
                        } else {
                            c.make_ascii_lowercase()
                        };
                    }
                }
            }

            let rec = SeqQualRecord::new(&record, &seq, None);
            format_writer.write(&rec, io_writer, ctx)?;

            Ok(true)
        })
    })
    .map(|r| Some(Report::to_box(r)))
}
