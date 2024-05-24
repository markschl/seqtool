use std::fmt::Debug;

mod approx;
mod exact;
mod regex;

use bio::alignment::AlignmentOperation;

use crate::{cmd::find::opts::Algorithm, error::CliResult};

use super::opts::Opts;

pub trait Matcher: Debug {
    fn has_matches(&self, text: &[u8]) -> Result<bool, String>;

    /// This method iterates over all hits and provides these to the
    /// given closure. The exact hit type may vary depending on the
    /// implementation.
    /// The looping should be interrupted if the closure returns false.
    fn iter_matches(
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
    pattern: &str,
    algorithm: Algorithm,
    ambig: bool,
    opts: &Opts,
) -> CliResult<Box<dyn Matcher + Send>> {
    use Algorithm::*;
    if algorithm != Regex && opts.has_groups() {
        return fail!(
            "Match groups > 0 can only be used with regular expression searches (-r/--regex or --regex-unicode)."
        );
    }
    let matcher: Box<dyn Matcher + Send> = match algorithm {
        Exact => Box::new(exact::ExactMatcher::new(pattern.as_bytes())),
        Regex => regex::get_matcher(pattern, opts.has_groups())?,
        Myers => approx::get_matcher(pattern, ambig, opts)?,
    };
    Ok(matcher)
}
