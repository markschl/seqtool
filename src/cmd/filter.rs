use std::fs::File;
use std::io::BufWriter;

use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::var::{func::Func, symbols::Value};

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

pub fn run(mut cfg: Config, args: &FilterCommand) -> CliResult<()> {
    let expr = args.expression.trim();
    if expr.starts_with('{') && expr.ends_with('}') {
        eprintln!(
            "Warning: found filter expression in the form {{ expression }}. \
            The double brackets are unnecessary and should be removed for the \
            expression to work properly."
        )
    }
    let dropped_file = args.dropped.as_ref();

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        let func = Func::expr(expr);
        let (expr_id, _, _) = cfg.build_vars(|b| b.register_var(&func))?.unwrap();
        let mut dropped_file = dropped_file
            .map(|f| Ok::<_, CliError>(BufWriter::new(File::create(f)?)))
            .transpose()?;
        cfg.read(|record, ctx| {
            let v = ctx.symbols.get(expr_id);
            let result = match v.inner() {
                Some(Value::Bool(b)) => *b.get(),
                _ => {
                    return fail!(
                        "Filter expression did not return a boolean (true/false), \
                    found '{}' instead",
                        v
                    )
                }
            };

            if result {
                format_writer.write(&record, io_writer, ctx)?;
            } else if let Some(w) = dropped_file.as_mut() {
                format_writer.write(&record, w, ctx)?;
            }
            Ok(true)
        })
    })
}
