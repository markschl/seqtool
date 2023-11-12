use crate::config;
use crate::error::CliResult;
use crate::io::DefRecord;
use crate::opt;
use crate::var::*;

pub static USAGE: &str = concat!(
    "
Deletes description field or attributes.

Usage:
    st del [options][-a <attr>...][-l <list>...] [<input>...]
    st del (-h | --help)
    st del --help-vars

Options:
    -d, --desc          Delete description
    --attrs <names>     Delete attributes (comma delimited list)
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    let del_desc = args.get_bool("--desc");
    let attrs = args.opt_str("--attrs");

    cfg.writer(|writer, vars| {
        if let Some(attrs) = attrs {
            vars.build(|b| {
                for p in attrs.split(',') {
                    b.register_attr(p, Some(attr::Action::Delete));
                }
                Ok(())
            })?;
        }

        cfg.read(vars, |record, vars| {
            if del_desc {
                let id = record.id_bytes();
                let record = DefRecord::new(&record, id, None);
                writer.write(&record, vars)?;
            } else {
                writer.write(&record, vars)?;
            }
            Ok(true)
        })
    })
}
