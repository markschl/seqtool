use std::path::PathBuf;

use clap::Parser;

use super::{MapFormat, UniqueVars};
use crate::cli::CommonArgs;
use crate::helpers::bytesize::parse_bytesize;
use crate::var::VarProvider;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct UniqueCommand {
    /// The key used to determine, which records are unique.
    /// The key can be a single variable/function such as 'seq',
    /// or a composed string such as '{attr(a)}_{attr(b)}'.
    /// For each key, the *first* encountered record is returned, and all
    /// remaining ones with the same key are discarded.
    pub key: String,

    /// Interpret the key as a number instead of text.
    /// De-replicating by a numeric header attribute or a field from associated
    /// metadata may be faster.
    /// Variables/functions such as `seqhash`, `gc`, `exp_err`, etc. are always numeric.
    #[arg(short, long)]
    pub numeric: bool,

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

    /// Silence any warnings
    #[arg(short, long)]
    pub quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn get_varprovider(_args: &UniqueCommand) -> Option<Box<dyn VarProvider>> {
    Some(Box::<UniqueVars>::default())
}
