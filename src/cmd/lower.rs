use cfg;
use error::CliResult;
use io::SeqQualRecord;
use opt;

static USAGE: &'static str = concat!(
    "
Converts all characters in the sequence to lowercase.

Usage:
    st lower [options][-a <attr>...] [-l <list>...] [<input>...]
    st lower (-h | --help)
    st lower --help-vars
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    cfg.writer(|writer, mut vars| {
        let mut seq = vec![];
        cfg.read_sequential_var(&mut vars, |record, vars| {
            seq.clear();
            for s in record.seq_segments() {
                seq.extend(s.iter().cloned().map(|ref mut b| {
                    b.make_ascii_lowercase();
                    *b
                }));
            }
            let ucase_rec = SeqQualRecord::new(&record, &seq, None);
            writer.write(&ucase_rec, vars)?;
            Ok(true)
        })
    })
}
