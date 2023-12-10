use clap::Parser;

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;
use crate::var::symbols::Value;
use crate::var::Func;

/// Filters sequences by a mathematical expression which may contain any variable.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct FilterCommand {
    /// Filter expression
    expression: String,
    /// Output file for sequences that were removed by filtering.
    /// The extension is autorecognized if possible, fallback
    /// is the input format.
    #[arg(short, long, value_name = "FILE")]
    dropped: Option<String>,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &FilterCommand) -> CliResult<()> {
    let expr = &args.expression;
    let dropped_file = args.dropped.as_ref();

    cfg.writer(|writer, mut vars| {
        let func = Func::expr(expr);
        let (expr_id, _) = vars.build(|b| b.register_var(&func))?.unwrap();
        let mut dropped_file = dropped_file
            .map(|s| cfg.other_writer(s, Some(&mut vars)))
            .transpose()?;

        cfg.read(vars, |record, vars| {
            let v = vars.symbols().get(expr_id);
            let result = match v.value() {
                Some(Value::Bool(b)) => *b.get(),
                _ => return fail!(format!("Filter expression did not return a boolean (true/false) value, found {} instead", v))
            };

            if result {
                writer.write(&record, vars)?;
            } else if let Some(w) = dropped_file.as_mut() {
                w.write(&record, vars)?;
            }
            Ok(true)
        })
    })
}
