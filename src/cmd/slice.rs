use cfg;
use error::CliResult;
use lib::util::parse_range;
use opt;

pub static USAGE: &'static str = concat!(
    "
Get a slice of the sequences within a defined range.

Usage:
    seqtool slice [options][-a <attr>...][-l <list>...] <range> [<input>...]
    seqtool slice (-h | --help)
    seqtool slice --help-vars

Options:
    <range>             Range in form 'start..end' or '..end' or 'start..'

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;
    let rng = args.get_str("<range>");

    cfg.writer(|writer, mut vars| {
        let range = parse_range(rng)?;

        // convert from 1-based to 0-based coordinates
        let mut start = range.0.unwrap_or(1);
        if start == 0 {
            return fail!("Select ranges are 1-based, zero is not a valid start value");
        }
        start -= 1;
        let end = range.1;

        let mut i = 0;

        cfg.read_sequential_var(&mut vars, |record, vars| {
            // if a start value was specified, skip records
            // was thinking about using Itertools::dropping(), but have to check for errors...
            if i >= start {
                if let Some(e) = end {
                    if i >= e {
                        return Ok(false);
                    }
                }
                writer.write(&record, vars)?;
            }
            i += 1;
            Ok(true)
        })
    })
}
