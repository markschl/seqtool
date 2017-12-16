use std::cmp::max;

use error::CliResult;
use opt;
use cfg;

pub static USAGE: &'static str = concat!("
Returns the last sequences of the input.

Usage:
    seqtool tail [options][-p <prop>...][-l <list>...] [<input>...]
    seqtool tail (-h | --help)
    seqtool tail --help-vars

Options:
    -n, --num-seqs <N>   Number of sequences to select [default: 10]

", common_opts!());


pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let n = args.get_str("--num-seqs");
    let n_select: usize = n.parse().map_err(|_| format!("Invalid number: {}", n))?;

    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    cfg.writer(|writer, mut vars| {
        // first count the sequences
        // There is no infrastructure for jumping in and starting a read anywhere
        // in the file...

        let mut n = 0;

        cfg.read_sequential(|_| {
            n += 1;
            Ok(true)
        })?;

        let mut i = 0;
        let select_from = max(n, n_select) - n_select;

        cfg.read_sequential_var(&mut vars, |record, vars| {
            i += 1;
            if i >= select_from {
                writer.write(&record, vars)?;
            }
            Ok(true)
        })
    })
}
