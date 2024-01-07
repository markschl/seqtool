use std::path::PathBuf;

use clap::Parser;

use crate::cli::CommonArgs;
use crate::helpers::bytesize::parse_bytesize;

/// De-replicate records, returning only unique ones.
///
/// The order of the records is the same as in the input unless the memory limit
/// is exceeded, in which case temporary files are used and the records are
/// sorted by the unique key. Specify -s/--sorted to always sort the output.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct UniqueCommand {
    /// The key used to determine, which records are unique.
    /// If not specified, records are de-replicated by the sequence.
    /// The key can be a single variable/function
    /// such as 'id', or a composed string, e.g. '{id}_{desc}'.
    /// For each key, the *first* encountered record is returned, and all
    /// remaining ones with the same key are discarded.
    #[arg(short, long, default_value = "seq")]
    pub key: String,

    /// Interpret the key as a number instead of text.
    /// This may improve performance if the key is numeric, which could occur with
    /// header attributes or fields from associated lists with metadata.
    #[arg(short, long)]
    pub numeric: bool,

    /// Sort the output by key.
    /// Without this option, the records are in input order if the memory limit
    /// is *not* exceeded, but will be sorte by key otherwise.
    #[arg(short, long)]
    pub sort: bool,

    /// Maximum amount of memory to use for de-duplicating.
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
