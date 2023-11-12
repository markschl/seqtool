use crate::config;
use crate::error::CliResult;
use crate::opt;

pub static USAGE: &str = concat!(
    "
Interleaves records of all files in the input. The records will returned in
the same order as the files.

Usage:
    st interleave [options][-a <attr>...][-l <list>...] [<input>...]
    st interleave (-h | --help)
    st interleave --help-vars

Options:
    -n, --no-id-check   Don't check if the IDs of the files match
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    let id_check = !args.get_bool("--no-id-check");

    cfg.writer(|writer, vars| {
        let mut id = vec![];

        cfg.read_alongside(|i, rec| {
            if id_check {
                let rec_id = rec.id_bytes();
                if i == 0 {
                    id.clear();
                    id.extend(rec_id);
                } else if rec_id != id.as_slice() {
                    return fail!(format!(
                        "ID of record #{} ({}) does not match the ID of the first one ({})",
                        i + 1,
                        String::from_utf8_lossy(rec_id),
                        String::from_utf8_lossy(&id)
                    ));
                }
            }
            writer.write(rec, vars)
        })
    })
}
