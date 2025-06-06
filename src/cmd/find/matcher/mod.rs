use std::fmt::Debug;

use bio::alignment::AlignmentOperation;

use crate::error::CliResult;

use super::opts::{Algorithm, PatternConfig, SearchConfig, SearchOpts};

pub mod approx;
pub mod exact;
pub mod regex;

pub trait Matcher: Debug + MatcherBoxClone {
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

pub trait MatcherBoxClone {
    fn clone_box(&self) -> Box<dyn Matcher + Send + Sync>;
}

impl<T> MatcherBoxClone for T
where
    T: 'static + Matcher + Clone + Send + Sync,
{
    fn clone_box(&self) -> Box<dyn Matcher + Send + Sync> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Matcher + Send + Sync> {
    fn clone(&self) -> Box<dyn Matcher + Send + Sync> {
        self.clone_box()
    }
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
    opts: &SearchOpts,
) -> CliResult<Box<dyn Matcher + Send + Sync>> {
    use Algorithm::*;
    if cfg.algorithm != Regex && opts.has_regex_groups {
        return fail!(
            "Match groups > 0 can only be used with regular expression searches (-r/--regex or --regex-unicode)."
        );
    }
    let matcher: Box<dyn Matcher + Send + Sync> = match cfg.algorithm {
        Exact => Box::new(exact::ExactMatcher::new(cfg.pattern.seq.as_bytes())),
        Regex => regex::get_matcher(
            &cfg.pattern.seq,
            opts.hit_limit <= 1,
            opts.has_regex_groups,
            opts.case_insensitive,
        )?,
        Myers => approx::get_matcher(&cfg.pattern.seq, cfg.max_dist, cfg.has_ambigs, opts)?,
    };
    Ok(matcher)
}

pub fn get_matchers(cfg: &SearchConfig) -> CliResult<Vec<Box<dyn Matcher + Send + Sync>>> {
    let opts = cfg.get_opts();
    cfg.patterns()
        .iter()
        .map(|p| get_matcher(p, opts))
        .collect::<CliResult<Vec<_>>>()
}
