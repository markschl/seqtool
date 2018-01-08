use error::CliResult;
use opt;
use cfg;

pub static USAGE: &'static str = concat!("
Returns the first sequences of the input.

Usage:
    seqtool head [options][-a <attr>...][-l <list>...] [<input>...]
    seqtool head (-h | --help)
    seqtool head --help-vars

Options:
    -n, --num-seqs <N>  Number of sequences to select [default: 10]

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let n = args.get_str("--num-seqs");
    let n: usize = n.parse().map_err(|_| format!("Invalid number: {}", n))?;

    cfg.writer(|writer, mut vars| {
        let mut i = 0;

        cfg.read_sequential_var(&mut vars, |record, vars| {
            if i >= n {
                return Ok(false);
            }
            writer.write(&record, vars)?;
            i += 1;
            Ok(true)
        })
    })
}
