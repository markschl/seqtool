use std::fs::File;
use std::io::BufWriter;

use clap::Parser;

use crate::config::Config;
use crate::error::{CliError, CliResult};
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
    /// The output format is (currently) the same as for the main output,
    /// regardless of the file extension.
    // TODO: allow autorecognition of extension
    #[arg(short, long, value_name = "FILE")]
    dropped: Option<String>,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &FilterCommand) -> CliResult<()> {
    let expr = &args.expression;
    let dropped_file = args.dropped.as_ref();

    cfg.writer(|writer, io_writer, vars| {
        let func = Func::expr(expr);
        let (expr_id, _, _) = vars.build(|b| b.register_var(&func))?.unwrap();
        let mut dropped_file = dropped_file
            .map(|f| Ok::<_, CliError>(BufWriter::new(File::create(f)?)))
            .transpose()?;

        cfg.read(vars, |record, vars| {
            let v = vars.symbols().get(expr_id);
            let result = match v.inner() {
                Some(Value::Bool(b)) => *b.get(),
                _ => return fail!(format!("Filter expression did not return a boolean (true/false) value, found {} instead", v))
            };

            if result {
                writer.write(&record, io_writer, vars)?;
            } else if let Some(w) = dropped_file.as_mut() {
                writer.write(&record, w, vars)?;
            }
            Ok(true)
        })
    })
}
