use std::path::PathBuf;

use clap::Parser;

use crate::cli::CommonArgs;
use crate::helpers::bytesize::parse_bytesize;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Sort' command options")]
pub struct SortCommand {
    /// The key used to sort the records. It can be a single variable/function
    /// such as 'seq', 'id', a composed string, e.g. '{id}_{desc}',
    /// or a comma-delimited list of multiple variables/functions to sort by,
    /// e.g. 'seq,attr(a)'. In this case, the records are first sorted by sequence,
    /// but in case of identical sequences, records are sorted by the header
    /// attribute 'a'.
    ///
    /// To sort by a numeric FASTA attribute in the form '>id;size=123':
    /// `st sort 'num(attr(size))' --attr-fmt ';key=value' input.fasta`.
    ///
    /// Regarding formulas returning mixed text/numbers, the sorted records with
    /// text keys will be returned first and the sorted number records after them.
    /// Furthermore, NaN and missing values (null/undefined in JS expressions,
    /// missing `opt_attr()` values or missing entries in associated metadata)
    /// will appear last.
    pub key: String,

    /// Sort in reverse order
    #[arg(short, long)]
    pub reverse: bool,

    /// Maximum amount of memory (approximate) to use for sorting.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    pub max_mem: usize,

    /// Path to temporary directory (only if memory limit is exceeded)
    #[arg(long, value_name = "PATH")]
    pub temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, value_name = "N", default_value_t = 1000)]
    pub temp_file_limit: usize,

    #[command(flatten)]
    pub common: CommonArgs,
}
