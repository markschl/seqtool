use std::f64::NAN;

use cfg;
use error::CliResult;
use opt;

use lib::inner_result::MapRes;

pub static USAGE: &'static str = concat!(
    "
Filters sequences by a mathematical expression which may contain any variable.

Usage:
    seqtool filter [options][-a <attr>...][-l <list>...] <expression> [<input>...]
    seqtool filter (-h | --help)
    seqtool filter --help-vars

Options:
    --dropped <file>    Output file for sequences that were removed by filtering.
                        The extension is autorecognized if possible, fallback
                        is the input format.

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;
    let expr = args.get_str("<expression>");
    let dropped_file = args.opt_str("--dropped");

    cfg.writer(|writer, mut vars| {
        let expr_id = vars.build(|b| b.register_with_prefix(Some("expr_"), expr))?;
        let mut dropped_file =
            dropped_file.map_res(|s| cfg.other_writer(s, Some(&mut vars), None))?;

        cfg.read_sequential_var(&mut vars, |record, vars| {
            let result = vars
                .symbols()
                .get_float(expr_id)?
                .expect("Bug: expression value not in symbol table!");

            if result == 1. {
                writer.write(&record, vars)?;
            } else if result == 0. {
                if let Some(w) = dropped_file.as_mut() {
                    w.write(&record, vars)?;
                }
            } else if result == NAN {
                // cannot use match because of NAN
                return fail!(format!(
                    "Undefined result of math expression for record '{}'",
                    String::from_utf8_lossy(record.id_bytes())
                ));
            } else {
                return fail!(format!(
                    "Math expressions may only return false (0) or true (1), but the returned value is {}.",
                    result
                ));
            }
            Ok(true)
        })
    })
}
