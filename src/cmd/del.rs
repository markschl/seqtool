use clap::Parser;

use crate::cli::{CommonArgs, Report};
use crate::config::Config;
use crate::error::CliResult;
use crate::io::HeaderRecord;
use crate::var::attr;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Del' command options")]
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

pub fn run(mut cfg: Config, args: DelCommand) -> CliResult<Option<Box<dyn Report>>> {
    let del_desc = args.desc;
    let del_attrs = args.attrs.as_deref();

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        if let Some(attrs) = del_attrs {
            cfg.build_vars(|b| {
                for attr in attrs {
                    b.register_attr(attr, Some(attr::AttrWriteAction::Delete))?;
                }
                Ok::<_, String>(())
            })?;
        }

        cfg.read(|record, ctx| {
            if del_desc {
                let id = record.id();
                let record = HeaderRecord::new(&record, id, None);
                format_writer.write(&record, io_writer, ctx)?;
            } else {
                format_writer.write(&record, io_writer, ctx)?;
            }
            Ok(true)
        })
    })
    .map(|r| Some(Report::to_box(r)))
}
