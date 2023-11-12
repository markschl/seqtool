use csv;

use crate::config;
use crate::error::CliResult;
use crate::opt;
use crate::var::Func;

static USAGE: &str = concat!(
    "
Returns per sequence statistics as tab delimited list. All variables
(seqlen, exp_err, charcount(...), etc.) can be used (see `st stat --help-vars`).
The command is equivalent to `st . --to-tsv 'id,var1,var2,...' input`

Usage:
    st stat <vars> [<input>...]
    st stat (-h | --help)

Options:
    <vars>             Comma delimited list of statistics variables.
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;
    let stats: Vec<_> = args.get_str("<stats>").split(',').collect();

    // TODO: can be further simplified (--to-tsv...)
    cfg.io_writer(|writer, vars| {
        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(writer);

        let var_ids: Vec<usize> = vars.build(|b| {
            stats
                .iter()
                .map(|s| {
                    b.register_var_or_fail(&Func::var(s.to_string()))
                        .map(|(id, _)| id)
                })
                .collect::<CliResult<_>>()
        })?;

        // header
        csv_writer.write_record(Some("id").iter().chain(&stats))?;

        // CSV row
        let mut row = vec![vec![]; stats.len() + 1];

        cfg.read(vars, |record, vars| {
            {
                let mut row_iter = row.iter_mut();
                let id = row_iter.next().unwrap();
                id.clear();
                id.extend_from_slice(record.id_bytes());

                for (&var_id, ref mut field) in var_ids.iter().zip(row_iter) {
                    field.clear();
                    vars.symbols()
                        .get(var_id)
                        .as_text(record, |s| field.extend_from_slice(s));
                }
            }

            csv_writer.write_record(&row)?;

            Ok(true)
        })
    })
}
