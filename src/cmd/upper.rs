use crate::config;
use crate::error::CliResult;
use crate::io::SeqQualRecord;
use crate::opt;

static USAGE: &str = concat!(
    "
Converts all characters in the sequence to uppercase.

Usage:
    st upper [options][-a <attr>...] [-l <list>...] [<input>...]
    st upper (-h | --help)
    st upper --help-vars
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    cfg.writer(|writer, vars| {
        let mut seq = vec![];
        cfg.read(vars, |record, vars| {
            seq.clear();
            for s in record.seq_segments() {
                seq.extend(s.iter().cloned().map(|ref mut b| {
                    b.make_ascii_uppercase();
                    *b
                }));
            }
            let ucase_rec = SeqQualRecord::new(&record, &seq, None);
            writer.write(&ucase_rec, vars)?;
            Ok(true)
        })
    })
}
