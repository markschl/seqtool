use error::CliResult;
use io::{Record, SeqQualRecord};
use lib::rng::*;
use opt;

use cfg;

pub static USAGE: &'static str = concat!(
    "
Trims sequences to a given range.

Usage:
  st trim [options][-a <attr>...][-l <list>...] <range> [<input>...]
  st trim (-h | --help)
  st trim --help-vars

Options:
    <range>             Range in the form 'start..end' or 'start..' or '..end',
                        Variables containing one range bound or the whole range
                        are possible.
    -e, --exclude       Exclusive trim range: excludes start and end positions
                        from the output sequence.
    -0                  Interpret range as 0-based, with the end not included.
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let range = args.get_str("<range>");
    let rng0 = args.get_bool("-0");
    let exclusive = args.get_bool("--exclude");

    cfg.writer(|writer, mut vars| {
        let mut out_seq = vec![];
        let mut out_qual = vec![];

        let mut rng = VarRange::from_str(range, &mut vars)?;

        cfg.read_sequential_var(&mut vars, |record, vars| {
            let seqlen = record.seq_len();

            let (start, end) = rng.get(seqlen, rng0, exclusive, vars.symbols())?;

            let rec = trim(&record, start, end, &mut out_seq, &mut out_qual);

            writer.write(&rec, vars)?;
            Ok(true)
        })
    })
}

fn trim<'r>(
    record: &'r Record,
    start: usize,
    end: usize,
    out_seq: &'r mut Vec<u8>,
    out_qual: &'r mut Vec<u8>,
) -> SeqQualRecord<'r, &'r Record> {
    out_seq.clear();

    if let Some(qual) = record.qual() {
        // no multiline sequence (FASTQ)
        let seq = record.raw_seq();

        out_qual.clear();

        out_seq.extend_from_slice(&seq[start..end]);
        out_qual.extend_from_slice(&qual[start..end]);
        SeqQualRecord::new(record, out_seq, Some(out_qual))
    } else {
        let mut s = start;
        let mut e = end;

        for seq in record.seq_segments() {
            if s >= seq.len() {
                // skip line
                s -= seq.len();
                e -= seq.len();
                continue;
            }

            if e < seq.len() {
                // stop at this line
                out_seq.extend_from_slice(&seq[s..e]);
                break;
            }

            out_seq.extend_from_slice(&seq[s..]);

            s = 0;
            e -= seq.len();
        }
        SeqQualRecord::new(record, out_seq, None)
    }
}
