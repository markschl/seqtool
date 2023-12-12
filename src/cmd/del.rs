use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::io::DefRecord;
use crate::opt::CommonArgs;
use crate::var::*;

/// Deletes description field or attributes
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct DelCommand {
    /// Delete description fields
    #[arg(short, long)]
    desc: bool,

    /// Delete attributes
    #[arg(long, value_delimiter = ',')]
    attrs: Option<Vec<String>>,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &DelCommand) -> CliResult<()> {
    let del_desc = args.desc;
    let del_attrs = args.attrs.as_deref();

    cfg.writer(|writer, io_writer, vars| {
        if let Some(attrs) = del_attrs {
            vars.build(|b| {
                for attr in attrs {
                    b.register_attr(attr, Some(attr::Action::Delete));
                }
                Ok(())
            })?;
        }

        cfg.read(vars, |record, vars| {
            if del_desc {
                let id = record.id_bytes();
                let record = DefRecord::new(&record, id, None);
                writer.write(&record, io_writer, vars)?;
            } else {
                writer.write(&record, io_writer, vars)?;
            }
            Ok(true)
        })
    })
}
