use itertools::Itertools;
use strum_macros::Display;

use crate::config::Config;
use crate::helpers::{
    rng::Range,
    seqtype::{guess_seqtype_or_fail, SeqType, SeqTypeInfo},
    DefaultHashSet as HashSet,
};
use crate::io::RecordAttr;
use crate::CliResult;

use super::cli::FindCommand;
use super::matcher::{get_matcher, Matcher};
use super::matches::Matches;

/// General options/properties derived from CLI options
#[derive(Debug)]
pub struct Opts {
    // pattern-related information
    pub patterns: Vec<String>,
    pub pattern_names: Vec<Option<String>>,
    pub algorithms: Vec<(Algorithm, bool)>,
    // required information
    // (group 0 = full hit)
    pub groups: Vec<usize>,
    pub required_info: RequiredInfo,
    pub max_hits: usize, // specify usize::MAX for unlimited
    // where and how to search
    pub attr: RecordAttr,
    pub in_order: bool,
    pub max_dist: Option<DistanceThreshold>,
    pub seqtype: SeqType,
    pub bounds: Option<Range>,
    pub max_shift: Option<Shift>,
    pub gap_penalty: u32,
    // actions
    pub filter: Option<bool>,
    pub dropped_path: Option<String>,
}

impl Opts {
    pub fn new(cfg: &mut Config, args: &FindCommand) -> CliResult<Self> {
        // search options
        let max_dist = if let Some(d) = args.search.max_diffs {
            Some(DistanceThreshold::Diffs(d))
        } else if let Some(d) = args.search.max_diff_rate {
            if !(0.0..=1.0).contains(&d) {
                return fail!(
                    "The maximum fraction of diverging letters (`-R/--max-diff-rate`) \
                    must be between 0 and 1"
                );
            }
            Some(DistanceThreshold::DiffRate(d))
        } else {
            None
        };
        // required information: will be updated later based on CLI/variables,
        // default if only filtering is 'exists'
        let mut required_info = RequiredInfo::Exists;
        let mut max_hits = 0;
        let mut groups = Vec::new();

        // search range
        let bounds = args
            .search_range
            .rng
            .map(|rng| rng.adjust(false, false))
            .transpose()?;

        let max_shift = if let Some(n) = args.search_range.max_shift_start {
            Some(Shift::Start(n))
        } else {
            args.search_range.max_shift_end.map(Shift::End)
        };
        if max_shift.is_some() {
            required_info = RequiredInfo::Range;
            groups.push(0);
            max_hits = 1;
        }

        // what should be searched?
        let attr = if args.attr.id {
            RecordAttr::Id
        } else if args.attr.desc {
            RecordAttr::Desc
        } else {
            RecordAttr::Seq
        };
        // search "actions"
        let filter = if args.action.filter {
            Some(true)
        } else if args.action.exclude {
            if args.action.filter {
                return fail!("-f/--filter and -e/--exclude cannot both be specified");
            }
            Some(false)
        } else {
            None
        };

        // Obtain a sequence type and search algorithm for each pattern
        // (based on heuristic and/or CLI args)
        let (seqtype, algorithms) =
            analyse_patterns(args, cfg.input_opts()[0].1.seqtype, attr, max_dist)?;

        let (pattern_names, patterns): (Vec<_>, Vec<_>) = args.patterns.iter().cloned().unzip();

        Ok(Self {
            patterns,
            pattern_names,
            algorithms,
            groups,
            required_info,
            max_hits,
            attr,
            in_order: args.search.in_order,
            max_dist,
            seqtype,
            bounds,
            max_shift,
            gap_penalty: args.search.gap_penalty,
            filter,
            dropped_path: args.action.dropped.clone(),
        })
    }

    pub fn has_groups(&self) -> bool {
        self.groups.iter().any(|g| *g > 0)
    }

    pub fn get_matchers(&self) -> CliResult<Vec<Box<dyn Matcher + Send>>> {
        self.algorithms
            .iter()
            .zip(&self.patterns)
            .map(|(&(algo, is_ambig), patt)| get_matcher(patt, algo, is_ambig, self))
            .collect::<CliResult<Vec<_>>>()
    }

    pub fn init_matches(&self) -> Matches {
        Matches::new(
            self.pattern_names.clone(),
            self.patterns.clone(),
            self.groups.clone(),
            self.max_hits,
            self.max_shift,
            self.bounds,
        )
    }
}

/// Required information based on CLI options / variables (functions).
/// Each additional variant includes all earlier ones
/// (`Alignment` requires range, edit distance and presence of a hit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequiredInfo {
    Exists,
    Distance,
    Range,
    Alignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Display)]
pub enum Algorithm {
    Exact,
    Regex,
    Myers,
}

pub fn algorithm_from_name(s: &str) -> Result<Option<Algorithm>, String> {
    match &*s.to_ascii_lowercase() {
        "exact" => Ok(Some(Algorithm::Exact)),
        "regex" => Ok(Some(Algorithm::Regex)),
        "myers" => Ok(Some(Algorithm::Myers)),
        "auto" => Ok(None),
        _ => Err(format!("Unknown search algorithm: {}", s)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistanceThreshold {
    Diffs(usize),
    DiffRate(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shift {
    Start(usize),
    End(usize),
}

impl Shift {
    pub fn in_range(&self, rng: (usize, usize), len: usize) -> bool {
        match *self {
            Shift::Start(n) => rng.0 <= n,
            Shift::End(n) => {
                if let Some(diff) = len.checked_sub(rng.1) {
                    diff <= n
                } else {
                    panic!("Range end greater than len ({} > {})", rng.1, len);
                }
            }
        }
    }
}

fn analyse_patterns(
    args: &FindCommand,
    typehint: Option<SeqType>,
    search_attr: RecordAttr,
    max_dist: Option<DistanceThreshold>,
) -> CliResult<(SeqType, Vec<(Algorithm, bool)>)> {
    let mut ambig_seqs = vec![];
    let quiet = args.common.general.quiet;

    let (unique_seqtypes, out): (HashSet<SeqType>, Vec<(Algorithm, bool)>) = args
        .patterns
        .iter()
        .map(|(name, pattern)| {
            let info = if args.search.regex {
                SeqTypeInfo::new(SeqType::Other, false, false)
            } else {
                guess_seqtype_or_fail(pattern.as_bytes(), typehint, true).map_err(|e| {
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
            let mut has_ambig = info.has_wildcard || info.has_ambiguities;
            if has_ambig {
                ambig_seqs.push(name);
            }
            // override if no_ambig was set
            if args.search.no_ambig {
                has_ambig = false;
            }

            // decide which algorithm should be used
            let mut algorithm = if args.search.regex {
                Algorithm::Regex
            } else if max_dist.is_some() || has_ambig {
                Algorithm::Myers
            } else {
                Algorithm::Exact
            };

            // override with user choice
            if let Some(a) = args.search.algo {
                algorithm = a;
                if a != Algorithm::Myers && has_ambig {
                    eprintln!("Warning: `--ambig` ignored with search algorithm '{}'.", a);
                    has_ambig = false;
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
                if has_ambig {
                    eprint!(" (with ambiguous letters)");
                }
                eprintln!(
                    ". If incorrect, please provide the correct type with `--seqtype`. \
                    Use `-q/--quiet` to suppress this message."
                );
            }

            Ok((info.seqtype, (algorithm, has_ambig)))
        })
        .collect::<CliResult<Vec<_>>>()?
        .into_iter()
        .unzip();

    if args.search.no_ambig && !ambig_seqs.is_empty() && !quiet {
        eprintln!(
            "Warning: Ambiguous matching is deactivated (--no-ambig), but there are patterns \
            with ambiguous letters ({}). Use `-q/--quiet` to suppress this message.",
            ambig_seqs.iter().map(|s| s.as_ref().unwrap()).join(", ") // unwrap: >1 patterns means they are all named
        );
    }

    if out
        .iter()
        .any(|&(a, _)| a == Algorithm::Regex || a == Algorithm::Exact)
        && max_dist.is_some()
        && !quiet
    {
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

    let t = unique_seqtypes.into_iter().next().unwrap();
    Ok((t, out))
}
