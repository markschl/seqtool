use std::fmt::Debug;

use bio::alignment::AlignmentOperation;

use crate::error::CliResult;

use super::opts::{Algorithm, PatternConfig, SearchConfig, SearchOpts, SearchRequirements};

pub mod approx;
pub mod exact;
pub mod regex;

pub trait Matcher: Debug {
    fn has_matches(&self, text: &[u8]) -> Result<bool, String>;

    /// This method iterates over all hits and provides these to the
    /// given closure. The exact hit type may vary depending on the
    /// implementation.
    /// The looping should be interrupted if the closure returns false.
    fn do_search(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
    ) -> Result<(), String>;
}

pub trait Hit {
    fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String>;
}

/// contains 0-based coordinates and distance
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub dist: usize,
    pub alignment_path: Vec<AlignmentOperation>,
}

impl Match {
    pub fn neg_start1(&self, seq_len: usize) -> i64 {
        self.start as i64 - seq_len as i64
    }

    pub fn neg_end1(&self, seq_len: usize) -> i64 {
        self.end as i64 - seq_len as i64 - 1
    }
}

pub fn get_matcher(
    cfg: &PatternConfig,
    search_opts: &SearchOpts,
    requirements: &SearchRequirements,
) -> CliResult<Box<dyn Matcher + Send>> {
    use Algorithm::*;
    if cfg.algorithm != Regex && requirements.has_regex_groups {
        return fail!(
            "Match groups > 0 can only be used with regular expression searches (-r/--regex or --regex-unicode)."
        );
    }
    let matcher: Box<dyn Matcher + Send> = match cfg.algorithm {
        Exact => Box::new(exact::ExactMatcher::new(cfg.pattern.seq.as_bytes())),
        Regex => regex::get_matcher(
            &cfg.pattern.seq,
            requirements.max_hits <= 1,
            requirements.has_regex_groups,
        )?,
        Myers => approx::get_matcher(
            &cfg.pattern.seq,
            cfg.max_dist,
            cfg.has_ambigs,
            search_opts,
            requirements,
        )?,
    };
    Ok(matcher)
}

pub fn get_matchers(
    cfg: &SearchConfig,
    opts: &SearchOpts,
) -> CliResult<Vec<Box<dyn Matcher + Send>>> {
    let req = cfg.get_search_requirements();
    cfg.patterns()
        .iter()
        .map(|p| get_matcher(p, opts, &req))
        .collect::<CliResult<Vec<_>>>()
}
