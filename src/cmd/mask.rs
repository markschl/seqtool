
use error::CliResult;
use opt;
use io::SeqQualRecord;
use lib::rng::*;

use cfg;

pub static USAGE: &'static str = concat!("
Masks the sequence within a given range or comma delimited list of ranges
by converting to lowercase (soft mask) or replacing with a character (hard
masking). Reverting soft masking is also possible.

Usage:
    seqtool mask [options][-a <attr>...][-l <list>...] <ranges> [<input>...]
    seqtool mask (-h | --help)
    seqtool mask --help-vars

Options:
    <range>             Range in the form 'start..end' or 'start..' or '..end',
                        Variables containing one range bound or the whole range
                        are possible.
    --hard <C>          Do hard masking instead of soft masking, replacing
                        everything in the range(s) with the given character
    --unmask            Unmask (convert to uppercase instead of lowercase)
    -e, --exclude       Exclusive range: excludes start and end positions
                        from the masked sequence.
    -0                  Interpret range as 0-based, with the end not included.

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let ranges = args.get_str("<ranges>");
    let hard_mask = args.opt_str("--hard").map(|c| c.as_bytes()[0]);
    let rng0 = args.get_bool("-0");
    let exclusive = args.get_bool("--exclude");
    let unmask = args.get_bool("--unmask");

    cfg.writer(
        |writer, mut vars| {

            let mut ranges = VarRanges::from_str(ranges, &mut vars)?;
            let mut seq = vec![];

            cfg.read_sequential_var(&mut vars, |record, vars| {

                let seqlen = record.seq_len();

                seq.clear();
                for s in record.seq_segments() {
                    seq.extend_from_slice(s);
                }

                let calc_ranges = ranges.get(seqlen, rng0, exclusive, vars.symbols())?;

                if let Some(h) = hard_mask {
                    for &(start, end) in calc_ranges {
                        for c in &mut seq[start..end] {
                            *c = h;
                        }
                    }
                } else {
                    for &(start, end) in calc_ranges {
                        for c in &mut seq[start..end] {
                            if unmask {
                                c.make_ascii_uppercase()
                            } else {
                                c.make_ascii_lowercase()
                            };
                        }
                    }
                }

                writer.write(&SeqQualRecord::new(&record, &seq, None), vars)?;

                Ok(true)
            })
        },
    )
}
