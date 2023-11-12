use crate::config;
use crate::error::CliResult;
use crate::opt;
use crate::var::symbols::Value;
use crate::var::Func;

pub static USAGE: &str = concat!(
    "
Filters sequences by a mathematical expression which may contain any variable.

Usage:
    st filter [options][-a <attr>...][-l <list>...] <expression> [<input>...]
    st filter (-h | --help)
    st filter --help-vars

Options:
    --dropped <file>    Output file for sequences that were removed by filtering.
                        The extension is autorecognized if possible, fallback
                        is the input format.
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;
    let expr = args.get_str("<expression>");
    let dropped_file = args.opt_str("--dropped");

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
