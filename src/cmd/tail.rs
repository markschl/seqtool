use std::cmp::max;

use crate::config;
use crate::error::CliResult;
use crate::opt;

pub static USAGE: &str = concat!(
    "
Returns the last sequences of the input.

Usage:
    st tail [options][-a <attr>...][-l <list>...] [<input>...]
    st tail (-h | --help)
    st tail --help-vars

Options:
    -n, --num-seqs <N>   Number of sequences to select [default: 10]
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    let n = args.get_str("--num-seqs");
    let n_select: usize = n.parse().map_err(|_| format!("Invalid number: {}", n))?;

    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    cfg.writer(|writer, vars| {
        // first count the sequences
        // TODO: maybe support .fai files and use them?

        let mut n = 0;

        cfg.read_simple(|_| {
            n += 1;
            Ok(true)
        })?;

        let mut i = 0;
        let select_from = max(n, n_select) - n_select;

        cfg.read(vars, |record, vars| {
            i += 1;
            if i > select_from {
                writer.write(&record, vars)?;
            }
            Ok(true)
        })
    })
}
