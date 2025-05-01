use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::var::{modules::expr::js::parser::Expression, symbols::Value};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Filter' command options")]
pub struct FilterCommand {
    /// Filter expression
    expression: String,
    /// Output file for sequences that were removed by filtering.
    /// The format is auto-recognized from the extension.
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
            The surrounding brackets are unnecessary and should be removed for the \
            expression to work properly."
        )
    }

    let parsed_expr = Expression::parse(expr)?;
    let (symbol_id, _) = cfg.build_vars(move |b| b.register_expr(&parsed_expr))?;

    let mut dropped_out = args
        .dropped
        .as_ref()
        .map(|f| cfg.new_output(f))
        .transpose()?;

    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read(|record, ctx| {
            let v = ctx.symbols.get(symbol_id);
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
            } else if let Some((d_writer, d_format_writer)) = dropped_out.as_mut() {
                d_format_writer.write(&record, d_writer, ctx)?;
            }
            Ok(true)
        })?;
        if let Some((w, _)) = dropped_out {
            w.finish()?;
        }
        Ok(())
    })
}
