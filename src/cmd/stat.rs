use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::error::CliResult;
use crate::{config::Config, io::FormatVariant};

use super::pass::{self, PassCommand};

pub const DESC: &str = "\
Sequence statistics variables (seqlen, exp_err, charcount(...), etc.)
are supplied as comma-delimited list, e.g. `id,seqlen,exp_err`.
The stat command is equivalent to `st pass --to-tsv 'id,var1,var2,...' input`

See `st stat --help-vars` for a list of all possible variables.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Stat' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct StatCommand {
    /// Comma delimited list of statistics variables.
    #[arg(value_name = "VAR")]
    vars: String,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(_cfg: Config, args: &StatCommand) -> CliResult<()> {
    let mut cmd = PassCommand {
        common: args.common.clone(),
    };
    cmd.common.output.to_tsv = Some("id,".to_string() + &args.vars);
    if let Some(info) = cmd.common.output.to.as_mut() {
        info.format = FormatVariant::Tsv;
    }
    pass::run(Config::new(&cmd.common)?, &cmd)
}
