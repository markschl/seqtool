use std::path::PathBuf;

use clap::Parser;

use crate::cli::CommonArgs;
use crate::helpers::bytesize::parse_bytesize;

/// Sort records by sequence or any other criterion.
///
/// The sort key can be 'seq', 'id', or any variable/function, expression, or
/// text containing them (see <KEY> help).
/// 
/// Records are sorted in memory, it is up to the user of this function
/// to ensure that the whole input will fit into memory.
/// The default sort is by sequence.
///
/// The actual value of the key is available through the 'key' variable. It can
/// be written to a header attribute or TSV field.
/// This may be useful with JavaScript expressions, whose evaluation takes time,
/// and whose result should be written to the headers, e.g.:
/// 'st sort -n '{{ id.substring(3, 5) }}' -a id_num='{key}' input.fasta'
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SortCommand {
    /// The key used to sort the records. It can be a single variable/function
    /// such as 'seq', 'id', or a composed string, e.g. '{id}_{desc}'.
    /// To sort by a FASTA/FASTQ attribute in the form '>id;size=123', specify
    /// 'attr(size)' --numeric.
    /// Regarding formulas returning mixed text/numbers, the sorted records with
    /// text keys will be returned first and the sorted number records after them.
    /// Furthermore, NaN and missing values (null/undefined in JS expressions,
    /// missing `opt_attr()` values or missing entries in associated metadata)
    /// will appear last.
    pub key: String,

    /// Interpret the key as a number instead of text.
    /// If not specified, the variable type determines, whether the key
    /// is numeric or not.
    /// However, header attributes or fields from associated lists with metadata
    /// may also need to be interpreted as a number, which can be done by
    /// specifying --numeric.
    #[arg(short, long)]
    pub numeric: bool,

    /// Sort in reverse order
    #[arg(short, long)]
    pub reverse: bool,

    /// Maximum amount of memory to use for sorting.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    pub max_mem: usize,

    /// Path to temporary directory (only if memory limit is exceeded)
    #[arg(long)]
    pub temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, default_value_t = 1000)]
    pub temp_file_limit: usize,

    /// Silence any warnings
    #[arg(short, long)]
    pub quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}
