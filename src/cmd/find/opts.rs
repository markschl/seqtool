use strum_macros::Display;

use crate::{
    config::Config,
    helpers::{rng::Range, seqtype::SeqType},
    io::RecordAttr,
    var::varstring::VarString,
    CliResult,
};

use super::{
    cli::FindCommand,
    matcher::{get_matcher, Matcher},
    matches::Matches,
    vars::FindVars,
};

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
    pub replacement: Option<VarString>,
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
        let (seqtype, algorithms) = super::helpers::analyse_patterns(
            &args.patterns,
            args.search.algo,
            cfg.get_seqtype(),
            args.search.no_ambig,
            args.search.regex,
            max_dist,
            args.common.general.quiet,
        )?;

        // Parse replacement strings
        // These can contain variables/expressions.
        let replacement = args
            .action
            .rep
            .as_deref()
            .map(|text| {
                cfg.with_command_vars(|v, _| {
                    let match_vars: &mut FindVars = v.unwrap();
                    // For pattern replacement, *all* hits for group 0 (the full hit)
                    // up to the given max. edit distance must be known, since
                    // all of them will be replaced.
                    match_vars.register_all(0);
                    Ok::<_, String>(())
                })?;
                let (s, _) = cfg.build_vars(|b| VarString::parse_register(text, b, true))?;
                Ok::<_, String>(s)
            })
            .transpose()?;

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
            replacement,
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
