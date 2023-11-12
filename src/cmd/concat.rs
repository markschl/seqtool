use std::iter::repeat;

use crate::config;
use crate::error::CliResult;
use crate::io::OwnedRecord;
use crate::opt;

pub static USAGE: &str = concat!(
    "
Concatenates sequences/alignments from different files in the order
in which they are provided. Fails if the IDs don't match.

Usage:
    st concat [options][-a <attr>...][-l <list>...] [<input>...]
    st concat (-h | --help)
    st concat --help-vars

Options:
    -n, --no-id-check   Don't check if the IDs of the records from
                        the different files match
    -s, --spacer <N>    Add a spacer of <N> characters inbetween the concatenated
                        sequences.
    -c, --s-char <C>    Character to use as spacer for sequences [default: N]
    --q-char <C>        Character to use as spacer for qualities. The default
                        value is the highest quality value for Illumina 1.8+/
                        Phred+33, since there is no autorecognition of the
                        FASTQ encoding [default: J]
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;
    let id_check = !args.get_bool("--no-id-check");
    let spacer_n = args.opt_value("--spacer")?;
    let s_char = args
        .get_str("--s-char")
        .as_bytes()
        .first()
        .ok_or("Sequence spacer character empty")?;
    let q_char = args
        .get_str("--q-char")
        .as_bytes()
        .first()
        .ok_or("Quality spacer character empty")?;

    cfg.writer(|writer, vars| {
        let mut record = OwnedRecord::default();
        let num_readers = cfg.num_readers();
        if num_readers == 0 {
            return fail!("Nothing to concatenate!");
        }
        let max_idx = num_readers - 1;

        cfg.read_alongside(|i, rec| {
            let rec_id = rec.id_bytes();

            if i == 0 {
                // initialize record
                record.id.clear();
                record.id.extend(rec_id);
                if let Some(d) = rec.desc_bytes() {
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
                let qual = record.qual.get_or_insert(Vec::new());
                if i == 0 {
                    qual.clear();
                }
                qual.extend(q);
            }

            // spacer
            if let Some(n) = spacer_n {
                if i < max_idx {
                    record.seq.extend(repeat(s_char).take(n));
                    if let Some(q) = record.qual.as_mut() {
                        q.extend(repeat(q_char).take(n));
                    }
                }
            }

            // write at last
            if i == max_idx {
                writer.write(&record, vars)?;
            }
            Ok(())
        })
    })
}
