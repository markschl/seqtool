use std::path::PathBuf;

use clap::Parser;

use super::MapFormat;
use crate::cli::{CommonArgs, WORDY_HELP};
use crate::helpers::bytesize::parse_bytesize;

pub const DESC: &str = "\
The unique key can be 'seq' or any variable/function, expression, or
text containing them (see <KEY> help and `st unique --help-vars`).

The order of the records is the same as in the input unless the memory limit
is exceeded, in which case temporary files are used and all remaining records
are sorted by the unique key. Use `-s/--sorted` to always sort the output
by key.";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Unique' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct UniqueCommand {
    /// The key used to determine, which records are unique.
    /// The key can be a single variable/function such as 'seq',
    /// a composed string such as '{attr(a)}_{attr(b)}',
    /// or a comma-delimited list of multiple variables/functions, whose
    /// values are all taken into account, e.g. 'seq,num(attr(a))'. In case of
    /// identical sequences, records are still de-replicated by the header
    /// attribute 'a'.
    /// The 'num()' function turns text values into numbers, which can
    /// speed up the de-replication.
    /// For each key, the *first* encountered record is returned, and all
    /// remaining ones with the same key are discarded.
    pub key: String,

    /// Sort the output by key.
    /// Without this option, the records are in input order if the memory limit
    /// is *not* exceeded, but are sorted by key otherwise.
    #[arg(short, long)]
    pub sort: bool,

    /// Write a map of all duplicate sequence IDs to the given file (or '-' for stdout).
    /// By default, a two-column mapping of sequence ID -> unique reference record ID
    /// is written (`long` format).
    /// More formats can be selected with `--map_format`.
    #[arg(long)]
    pub map_out: Option<PathBuf>,

    /// Column format for the duplicate map `--map-out` (use `--help` for details).
    #[arg(long, value_enum, default_value = "long")]
    pub map_fmt: MapFormat,

    /// Maximum amount of memory (approximate) to use for de-duplicating.
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
