use std::fmt;
use std::str::FromStr;

use clap::{value_parser, Args, Parser};
use itertools::Itertools;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::helpers::{
    rng::Range,
    seqtype::{guess_seqtype_or_fail, SeqType, SeqTypeInfo},
    DefaultHashSet as HashSet,
};
use crate::io::RecordAttr;
use crate::CliResult;

use super::opts::{
    algorithm_from_name, Algorithm, Anchor, FilterOpts, PatternConfig, SearchConfig, SearchOpts,
};

#[derive(Debug, Clone)]
pub struct Pattern {
    pub name: Option<String>,
    pub seq: String,
}

pub fn parse_patterns(pattern: &str) -> CliResult<Vec<Pattern>> {
    let mut patterns = Vec::new();
    if !pattern.starts_with("file:") {
        patterns.push(Pattern {
            name: None,
            seq: pattern.to_string(),
        });
    } else {
        use seq_io::fasta::*;
        let path = &pattern[5..];
        let mut reader = Reader::from_path(path)?;
        while let Some(r) = reader.next() {
            let r = r?;
            patterns.push(Pattern {
                name: Some(r.id()?.to_string()),
                seq: String::from_utf8(r.owned_seq())?,
            });
        }
        if patterns.is_empty() {
            return fail!("Pattern file is empty. Patterns should be in FASTA format.",);
        }
    };
    if patterns.iter().any(|p| p.seq.is_empty()) {
        return fail!("Empty pattern found");
    }
    Ok(patterns)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitScoring {
    pub match_: i8,
    pub mismatch: i8,
    pub gap: i8,
}

impl Default for HitScoring {
    fn default() -> Self {
        HitScoring {
            match_: 1,
            mismatch: -1,
            gap: -2,
        }
    }
}

impl fmt::Display for HitScoring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.match_, self.mismatch, self.gap)
    }
}

impl FromStr for HitScoring {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let elements: Vec<_> = s.split(',').collect();
        if elements.len() != 3 {
            return fail!("The scoring should be in the form: 'match,mismatch,gap'",);
        }
        #[cold]
        fn cnv_err(s: &str) -> String {
            format!("Score is not an integer number: {}", s)
        }
        let out = HitScoring {
            match_: elements[0].parse().map_err(|_| cnv_err(elements[0]))?,
            mismatch: elements[1].parse().map_err(|_| cnv_err(elements[1]))?,
            gap: elements[2].parse().map_err(|_| cnv_err(elements[2]))?,
        };
        if out.match_ < 0 || out.mismatch >= 0 || out.gap >= 0 {
            return fail!(
                "Match scores should not be negative and \
                mismatch/gap score should be positive",
            );
        }
        Ok(out)
    }
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
    pub patterns: std::vec::Vec<Pattern>,

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
    /// and backreferences (see https://docs.rs/regex/#syntax).
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

    /// Don't recognize DNA/RNA ambiguities (IUPAC) in patterns.
    #[arg(long)]
    pub no_ambig: bool,

    /// Override decision of algorithm for testing (regex/exact/myers/auto)
    // Using std::option::Option due to Clap oddity (https://github.com/clap-rs/clap/issues/4626)
    #[arg(long, value_name = "NAME", default_value = "auto", value_parser = algorithm_from_name)]
    pub algo: std::option::Option<Algorithm>,

    /// Scoring to use for prioritizing among multiple matches with the
    /// same starting position and an equally small edit distance.
    /// Should be provided in the form: `match,mismatch,gap`
    /// The default gap penalty of -2 leads to more concise alignments.
    /// A high gap penalty does *not* enforce ungapped alignments.
    /// Only perfect matches (`-D/--max-diffs 0`) are ungapped.
    #[arg(long, value_name = "Ma,Mi,Gap", default_value_t = HitScoring::default()) ]
    pub hit_scoring: HitScoring,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Search range")]
pub struct SearchRangeArgs {
    /// Search within the given range ('start:end', 'start:' or ':end').
    /// Using variables is not possible.
    #[arg(long, value_name = "RANGE", allow_hyphen_values = true)]
    pub rng: Option<Range>,

    /// Consider only matches anchored to the start of the sequence
    /// (or the search range `--rng`), whereby a the start of the match may be
    /// shifted towards the right by a maximum of <TOLERANCE> letters.
    /// Additional approximate matches further towards the end are not considered
    /// (even if better).
    #[arg(long, value_name = "TOLERANCE")]
    pub anchor_start: Option<usize>,

    /// Consider only matches anchored to the end of the sequence
    /// (or the search range `--rng`), whereby a the end of the match may be
    /// shifted towards the left by a maximum of <TOLERANCE> letters.
    /// Additional approximate matches further towards the start are not considered
    /// (even if better).
    #[arg(long, value_name = "TOLERANCE")]
    pub anchor_end: Option<usize>,
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
    /// The format is auto-recognized from the extension.
    #[arg(long, value_name = "FILE")]
    pub dropped: Option<String>,

    /// Replace by a string, which may also contain {variables/functions}.
    #[arg(long, value_name = "BY")]
    pub rep: Option<String>,
}

impl FindCommand {
    /// parses the arguments, thereby moving out some allocated data from `self`
    /// (calling this twice is not possible)
    pub fn parse(
        mut self,
        seqtype_hint: Option<SeqType>,
    ) -> CliResult<(SearchConfig, SearchOpts, FilterOpts)> {
        let quiet = self.common.general.quiet;

        // what should be searched?
        let attr = if self.attr.id {
            RecordAttr::Id
        } else if self.attr.desc {
            RecordAttr::Desc
        } else {
            RecordAttr::Seq
        };

        // assemble all patterns
        let mut unique_seqtypes = HashSet::default();
        let mut ambig_seqs = Vec::new();
        let search_args = &self.search;
        let pattern_cfg: Vec<_> = self
            .patterns
            .drain(..)
            .enumerate()
            .map(|(i, pattern)| {
                // search options
                let max_dist = if let Some(d) = self.search.max_diffs {
                    d
                } else if let Some(rate) = self.search.max_diff_rate {
                    if !(0.0..=1.0).contains(&rate) {
                        return fail!(
                            "The maximum fraction of diverging letters (`-R/--max-diff-rate`) \
                            must be between 0 and 1"
                        );
                    }
                    (rate * pattern.seq.len() as f64) as usize
                } else {
                    0
                };
                // Obtain a sequence type and search algorithm for each pattern
                // (based on heuristic and/or CLI args)
                let (seq_type, algorithm, has_ambigs) = analyse_pattern(
                    pattern.name.as_deref(),
                    pattern.seq.as_bytes(),
                    search_args,
                    seqtype_hint,
                    attr,
                    max_dist,
                    quiet,
                )?;
                if has_ambigs {
                    ambig_seqs.push(i);
                }
                unique_seqtypes.insert(seq_type);

                Ok(PatternConfig {
                    pattern,
                    max_dist,
                    algorithm,
                    has_ambigs,
                })
            })
            .collect::<CliResult<_>>()?;

        if self.search.no_ambig && !ambig_seqs.is_empty() && !quiet {
            eprintln!(
                "Warning: Ambiguous matching is deactivated (--no-ambig), but there are patterns \
                    with ambiguous letters ({}). Use `-q/--quiet` to suppress this message.",
                ambig_seqs
                    .iter()
                    .map(|i| pattern_cfg[*i].pattern.name.as_deref().unwrap_or_default())
                    .join(", ")
            );
        }

        let unnecessary_diffs = pattern_cfg.iter().any(|cfg| {
            matches!(cfg.algorithm, Algorithm::Regex | Algorithm::Exact) && cfg.max_dist > 0
        });
        if unnecessary_diffs && !quiet {
            eprintln!(
                "Warning: `-D/--max-diffs` option ignored with exact/regex matching. \
                Use `-q/--quiet` to suppress this message."
            );
        }

        if unique_seqtypes.len() > 1 {
            return fail!(format!(
                "Autorecognition of pattern sequence types suggests that there are \
                several different types ({}). Please specify the correct type with --seqtype",
                unique_seqtypes
                    .iter()
                    .map(|t| format!("{:?}", t))
                    .join(", ")
            ));
        }

        let seqtype = unique_seqtypes.into_iter().next().unwrap();

        let mut config = SearchConfig::new(pattern_cfg);

        // hit anchoring
        let anchor = if let Some(n) = self.search_range.anchor_start {
            Some(Anchor::Start(n))
        } else {
            self.search_range.anchor_end.map(Anchor::End)
        };
        if let Some(a) = anchor {
            config.set_anchor(a);
        }

        // search range
        if let Some(rng) = self.search_range.rng {
            let rng = rng.adjust(false, false)?;
            config.set_search_range(rng);
        }

        // search "actions"
        let filter = if self.action.filter {
            Some(true)
        } else if self.action.exclude {
            if self.action.filter {
                return fail!("-f/--filter and -e/--exclude cannot both be specified");
            }
            Some(false)
        } else {
            None
        };
        let filter_opts = FilterOpts {
            filter,
            dropped_path: self.action.dropped.take(),
        };

        let search_opts = SearchOpts {
            in_order: self.search.in_order,
            seqtype,
            hit_scoring: self.search.hit_scoring,
            attr,
            replacement: self.action.rep.take(),
            threads: self.search.threads,
        };

        Ok((config, search_opts, filter_opts))
    }
}

fn analyse_pattern(
    name: Option<&str>,
    pattern: &[u8],
    search_args: &SearchArgs,
    typehint: Option<SeqType>,
    search_attr: RecordAttr,
    max_dist: usize,
    quiet: bool,
) -> CliResult<(SeqType, Algorithm, bool)> {
    let info = if search_args.regex {
        SeqTypeInfo::new(SeqType::Other, false, false)
    } else {
        guess_seqtype_or_fail(pattern, typehint, true).map_err(|e| {
            format!(
                "Error in search pattern{}: {}",
                name.as_ref()
                    .map(|n| format!(" '{}'", n))
                    .unwrap_or_default(),
                e
            )
        })?
    };
    // no discrimination here
    let mut has_ambigs = info.has_wildcard || info.has_ambiguities;

    // override if no_ambig was set
    if search_args.no_ambig {
        has_ambigs = false;
    }

    // decide which algorithm should be used
    let mut algorithm = if search_args.regex {
        Algorithm::Regex
    } else if max_dist > 0 || has_ambigs {
        Algorithm::Myers
    } else {
        Algorithm::Exact
    };

    // override with user choice
    if let Some(a) = search_args.algo {
        algorithm = a;
        if a != Algorithm::Myers && has_ambigs {
            eprintln!("Warning: `--ambig` ignored with search algorithm '{}'.", a);
            has_ambigs = false;
        }
    }

    if search_attr == RecordAttr::Seq
        && typehint.is_none()
        && algorithm != Algorithm::Regex
        && !quiet
    {
        // unless 'regex' was specified, we must know the correct sequence type,
        // or there could be unexpected behaviour
        eprint!("Note: the sequence type of the pattern ",);
        if let Some(n) = name {
            eprint!("'{}' ", n);
        }
        eprint!("was determined as '{}'", info.seqtype);
        if has_ambigs {
            eprint!(" (with ambiguous letters)");
        }
        eprintln!(
            ". If incorrect, please provide the correct type with `--seqtype`. \
            Use `-q/--quiet` to suppress this message."
        );
    }

    Ok((info.seqtype, algorithm, has_ambigs))
}
