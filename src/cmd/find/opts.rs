use clap::{value_parser, Args, Parser};

use crate::{
    helpers::{rng::Range, seqtype::SeqType},
    opt::CommonArgs,
};

/// Fast searching for one or more patterns in sequences or ids/descriptions, with optional multithreading.
#[derive(Parser, Clone, Debug)]
pub struct FindCommand {
    // #[arg(required_unless_present = "--help-vars")]
    /// Pattern string or 'file:<patterns.fasta>'
    pub pattern: String,

    #[command(flatten)]
    pub search: SearchArgs,

    #[command(flatten)]
    pub search_range: SearchRangeArgs,

    #[command(flatten)]
    pub attr: SearchAttrArgs,

    #[command(flatten)]
    pub action: SearchActionArgs,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search options")]
pub struct SearchArgs {
    /// Treat the pattern(s) as regular expressions.
    #[arg(short, long)]
    pub regex: bool,

    /// Fuzzy string matching with maximum edit distance of <dist> [default: 0]
    #[arg(short, long, default_value_t = 0)]
    pub dist: usize,

    /// Report hits in the order of their occurrence instead of sorting by distance (with -d > 0)
    #[arg(long)]
    pub in_order: bool,

    /// Sequence type (auto-detect by default)
    #[arg(long)]
    pub seqtype: Option<SeqType>,

    /// Number of threads to use
    #[arg(short, long, value_name = "N", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    pub threads: u32,

    /// Don't interpret DNA ambiguity (IUPAC) characters.
    #[arg(long)]
    pub no_ambig: bool,

    /// Override decision of algorithm for testing (regex/exact/myers/auto) [default: auto]
    #[arg(long, value_name = "NAME", default_value = "auto")]
    pub algo: String,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search range")]
pub struct SearchRangeArgs {
    /// Search within the given range ('start..end', 'start..' or '..end'). Using variables is not possible.
    #[arg(long, value_name = "RANGE")]
    pub rng: Option<Range>,

    /// Consider only matches with a maximum distance of <n> from the search start (eventually > 1 if using --rng)
    #[arg(long, value_name = "N")]
    pub max_shift_l: Option<usize>,

    /// Consider only matches with a maximum distance from the end of the search range
    #[arg(long, value_name = "N")]
    pub max_shift_r: Option<usize>,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Where to search")]
pub struct SearchAttrArgs {
    /// Search / replace in IDs instead of sequences
    #[arg(short, long)]
    pub id: bool,

    /// Search / replace in descriptions
    #[arg(long)]
    pub desc: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search command actions")]
pub struct SearchActionArgs {
    /// Keep only matching sequences
    #[arg(short, long)]
    pub filter: bool,

    /// Exclude sequences that matched
    #[arg(short, long)]
    pub exclude: bool,

    /// Output file for sequences that were removed by filtering. The extension is autorecognized if possible, fallback is the input format.
    #[arg(long, value_name = "FILE")]
    pub dropped: Option<String>,

    /// Replace by a composable string
    #[arg(long, value_name = "BY")]
    pub rep: Option<String>,
}
