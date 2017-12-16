use error::CliResult;
use opt;
use io::DefRecord;
use var::*;

use cfg;

pub static USAGE: &'static str = concat!("
Deletes description field or properties.

Usage:
    seqtool del [options][-p <prop>...][-l <list>...] [<input>...]
    seqtool del (-h | --help)
    seqtool del --help-vars

Options:
    -d, --desc          Delete description
    --props <names>     Delete properties (comma delimited list)

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let del_desc = args.get_bool("--desc");
    let props = args.opt_str("--props");

    cfg.writer(|writer, mut vars| {
        if let Some(props) = props {
            vars.build(|b| {
                for p in props.split(',') {
                    b.register_prop(p, Some(prop::Action::Delete));
                }
                Ok(())
            })?;
        }

        cfg.read_sequential_var(&mut vars, |record, vars| {
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
