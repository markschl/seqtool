use std::path::PathBuf;

use clap::Parser;

use crate::cli::CommonArgs;
use crate::helpers::bytesize::parse_bytesize;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SortCommand {
    /// The key used to sort the records. It can be a single variable/function
    /// such as 'seq', 'id', a composed string, e.g. '{id}_{desc}',
    /// or a comma-delimited list of multiple variables/functions to sort by,
    /// e.g. 'seq,attr(a)'. In this case, the records are first sorted by sequence,
    /// but in case of identical sequences, records are sorted by the header
    /// attribute 'a'.
    ///
    /// To sort by a FASTA/FASTQ attribute in the form '>id;size=123', specify
    /// 'attr(size)' --numeric --attr-fmt ';key=value'.
    ///
    /// Regarding formulas returning mixed text/numbers, the sorted records with
    /// text keys will be returned first and the sorted number records after them.
    /// Furthermore, NaN and missing values (null/undefined in JS expressions,
    /// missing `opt_attr()` values or missing entries in associated metadata)
    /// will appear last.
    pub key: String,

    /// Interpret the key as a number instead of text.
    /// If not specified, the variable type determines, whether the key
    /// is numeric or not.
    /// Header attributes or fields from associated metadata may also need
    /// to be interpreted as a number, which can be done by pecifying --numeric.
    #[arg(short, long)]
    pub numeric: bool,

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

    /// Silence any warnings
    #[arg(short, long)]
    pub quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}
