use crate::config;
use crate::error::CliResult;
use crate::opt;

static USAGE: &str = concat!(
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
    let cfg = config::Config::from_args(&args)?;

    cfg.writer(|writer, vars| {
        cfg.read(vars, |record, vars| {
            writer.write(&record, vars)?;
            Ok(true)
        })
    })
}
