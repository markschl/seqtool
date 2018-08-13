use cfg;
use error::CliResult;
use opt;

static USAGE: &'static str = concat!(
    "
This command is useful for converting from one format to another
and/or setting attributes.

Usage:
    st (pass|.) [options][-a <attr>...][-l <list>...] [<input>...]
    st (pass|.) (-h | --help)
    st (pass|.) --help-vars
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    cfg.writer(|writer, mut vars| {
        cfg.read_sequential_var(&mut vars, |record, vars| {
            writer.write(&record, vars)?;
            Ok(true)
        })
    })
}
