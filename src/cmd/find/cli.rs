use clap::{value_parser, Args, Parser};

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::error::CliResult;
use crate::helpers::rng::Range;

use super::opts::{algorithm_from_name, Algorithm};

pub fn parse_patterns(pattern: &str) -> CliResult<Vec<(Option<String>, String)>> {
    let mut patterns = Vec::new();
    if !pattern.starts_with("file:") {
        patterns.push((None, pattern.to_string()))
    } else {
        use seq_io::fasta::*;
        let path = &pattern[5..];
        let mut reader = Reader::from_path(path)?;
        while let Some(r) = reader.next() {
            let r = r?;
            patterns.push((Some(r.id()?.to_string()), String::from_utf8(r.owned_seq())?));
        }
        if patterns.is_empty() {
            return fail!(
                "Pattern file is empty: {}. Patterns should be in FASTA format.",
                path
            );
        }
    };
    if patterns.iter().any(|(_, p)| p.is_empty()) {
        return fail!("Empty pattern found");
    }
    Ok(patterns)
}

pub const DESC: &str = "\
There are different search modes:

1. Exact search
2. Regular expressions (`-r/--regex`)
3. DNA or protein patterns with ambiguous letters
4. Approximate matching up to a given edit distance
    (`-D/--diffs` or `-R/--max-diff-rate`)

Search results can be used in three different ways:

1. Keeping (`-f/--filter`) or excluding (`-e/--exclude`) matched
   sequences
2. Pattern replacement (`--rep`) with ambiguous/approximate
   matching (for exact/regex replacement, use the 'replace'
   command)
3. Passing the search results to the output in sequence
   headers (`-a/--attr`) or TSV/CSV fields (`--to-tsv/--to-csv`);
   see `st find --help-vars` for all possible variables/
   functions
";

#[derive(Parser, Clone, Debug)]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct FindCommand {
    // #[arg(required_unless_present = "--help-vars")]
    /// Pattern string or 'file:<patterns.fasta>'
    // Using std::vec::Vec due to Clap oddity (https://github.com/clap-rs/clap/issues/4626)
    #[arg(value_parser = parse_patterns)]
    pub patterns: std::vec::Vec<(Option<String>, String)>,

    #[command(flatten)]
    pub attr: SearchAttrArgs,

    #[command(flatten)]
    pub search: SearchArgs,

    #[command(flatten)]
    pub search_range: SearchRangeArgs,

    #[command(flatten)]
    pub action: SearchActionArgs,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Where to search (default: sequence)")]
pub struct SearchAttrArgs {
    /// Search / replace in IDs instead of sequences
    #[arg(short, long)]
    pub id: bool,

    /// Search / replace in descriptions
    #[arg(short, long)]
    pub desc: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search options")]
pub struct SearchArgs {
    /// Return pattern matches up to a given maximum edit distance of N
    /// differences (= substitutions, insertions or deletions).
    /// Residues that go beyond the sequence (partial matches) are always
    /// counted as differences. [default: pefect match]
    #[arg(short = 'D', long, value_name = "N")]
    pub max_diffs: Option<usize>,

    /// Return of matches up to a given maximum rate of differences, that is
    /// the fraction of divergences (edit distance = substitutions, insertions or deletions)
    /// divided by the pattern length. If searching a 20bp pattern at a difference
    /// rate of 0.2, matches with up to 4 differences (see also `-D/--max-diffs`) are
    /// returned. [default: pefect match]
    #[arg(short = 'R', long, value_name = "R")]
    pub max_diff_rate: Option<f64>,

    /// Interpret pattern(s) as regular expression(s).
    /// All *non-overlapping* matches in are searched in headers or sequences.
    /// The regex engine lacks some advanced syntax features such as look-around
    /// and backreferences (see https://docs.rs/regex).
    /// Capture groups can be extracted by functions such as `match_group(number)`,
    /// or `match_group(name)` if named: `(?<name>)`
    /// (see also `st find --help-vars`).
    #[arg(short, long)]
    pub regex: bool,

    /// Report hits in the order of their occurrence instead of sorting by distance.
    /// Note that this option only has an effect with `-D/--max-dist` > 0, otherwise
    /// matches are always reported in the order of their occurrence.
    #[arg(long)]
    pub in_order: bool,

    /// Number of threads to use for searching
    #[arg(short, long, value_name = "N", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    pub threads: u32,

    /// Don't interpret DNA ambiguity (IUPAC) characters.
    #[arg(long)]
    pub no_ambig: bool,

    /// Override decision of algorithm for testing (regex/exact/myers/auto)
    // Using std::option::Option due to Clap oddity (https://github.com/clap-rs/clap/issues/4626)
    #[arg(long, value_name = "NAME", default_value = "auto", value_parser = algorithm_from_name)]
    pub algo: std::option::Option<Algorithm>,

    /// Gap penalty to use for selecting the the optimal alignment among multiple
    /// alignments with the same starting position and the same edit distance.
    /// The default penalty of 2 selects hits that don't have too InDels in the
    /// alignment.
    /// A penalty of 0 is not recommended; due to how the algorithm works,
    /// the alignment with the leftmost end position is chosen among the candidates,
    /// which often shows deletions in the pattern.
    /// Penalties >2 will further shift the preference towards hits with substitutions
    /// instead of InDels, but the selection is always done among hits with the
    /// same (lowest) edit distance, so raising the gap penalty will not help in
    /// trying to enfoce ungapped alignments (there is currently no way to do that).
    #[arg(long, value_name = "N", default_value_t = 2)]
    pub gap_penalty: u32,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search range")]
pub struct SearchRangeArgs {
    /// Search within the given range ('start:end', 'start:' or ':end'). Using variables is not possible.
    #[arg(long, value_name = "RANGE", allow_hyphen_values = true)]
    pub rng: Option<Range>,

    /// Consider only matches with a maximum of <N> letters preceding the start
    /// of the match (relative to the sequence start or the start of the range `--rng`)
    // TODO: adjust --rng to only search the range of possible occurrences
    #[arg(long, value_name = "N")]
    pub max_shift_start: Option<usize>,

    /// Consider only matches with a maximum of <N> letters following the end
    /// of the match (relative to the sequence end or the end of the range `--rng`)
    #[arg(long, value_name = "N")]
    pub max_shift_end: Option<usize>,
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

    /// Output file for sequences that were removed by filtering.
    /// The output format is (currently) the same as for the main output,
    /// regardless of the file extension.
    // TODO: allow autorecognition of extension
    #[arg(long, value_name = "FILE")]
    pub dropped: Option<String>,

    /// Replace by a string, which may also contain {variables/functions}.
    #[arg(long, value_name = "BY")]
    pub rep: Option<String>,
}
