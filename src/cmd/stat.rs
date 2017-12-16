use csv;

use error::CliResult;
use opt;
use cfg;

static USAGE: &'static str = concat!("
Returns per sequence statistics as tab delimited list. All statistical variables
(s:<variable>) can be used.

Usage:
    seqtool stat [options] <stats> [<input>...]
    seqtool stat (-h | --help)

Options:
    <stats>             Comma delimited list of statistics. The 's:' prefix
                        is not necessary.

",  common_opts!());


pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;
    let stats: Vec<_> = args.get_str("<stats>").split(',').collect();

    cfg.io_writer(|writer, mut vars| {
        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(writer);

        let var_ids: Vec<usize> = vars.build(|b| {
            stats
                .iter()
                .map(|s| b.register_with_prefix(Some("s"), s))
                .collect::<CliResult<_>>()
        })?;

        // header
        csv_writer.write_record(Some("id").iter().chain(&stats))?;

        // CSV row
        let mut row = vec![vec![]; stats.len() + 1];

        cfg.read_sequential_var(&mut vars, |record, vars| {
            {
                let mut row_iter = row.iter_mut();
                let id = row_iter.next().unwrap();
                id.clear();
                id.extend_from_slice(record.id_bytes());

                for (&var_id, ref mut field) in var_ids.iter().zip(row_iter) {
                    let val = vars.symbols().get_text(var_id).unwrap_or(b"");
                    field.clear();
                    field.extend_from_slice(val);
                }
            }

            csv_writer.write_record(&row)?;

            Ok(true)
        })
    })
}
