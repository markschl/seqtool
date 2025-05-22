use strum_macros::Display;
use vec_map::VecMap;

use crate::helpers::{rng::Range, seqtype::SeqType};
use crate::io::RecordAttr;

use super::cli::{HitScoring, Pattern};
use super::matcher::{regex::resolve_group, Match};
use super::matches::Matches;

#[derive(Debug, Clone)]
pub struct PatternConfig {
    pub pattern: Pattern,
    pub max_dist: usize,
    pub has_ambigs: bool,
    pub algorithm: Algorithm,
}

/// Required information based on CLI options / variables (functions).
/// Each additional variant requires that more information is collected,
/// and all the information required by earlier variants is also present.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequiredDetail {
    #[default]
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
        _ => Err(format!("Unknown search algorithm: {s}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    Start(usize),
    End(usize),
}

impl Anchor {
    /// Returns the possible search range for a pattern of given length in a text of given length.
    /// `pattern_len` should include the whole range that can be covered by the pattern
    /// (the longest possible gapped alignment in case of approximate matching).
    pub fn get_search_range(&self, pattern_len: usize, text_len: usize) -> (usize, usize) {
        match self {
            Anchor::Start(tolerance) => (0, text_len.min(*tolerance + pattern_len)),
            Anchor::End(tolerance) => (text_len.saturating_sub(pattern_len + *tolerance), text_len),
        }
    }

    /// Returns whether the range of a match falls within the specified anchor tolerance
    /// given the length of the text
    pub fn is_anchored(&self, match_rng: (usize, usize), text_len: usize) -> bool {
        match *self {
            Anchor::Start(tolerance) => match_rng.0 <= tolerance,
            Anchor::End(tolerance) => {
                if let Some(diff) = text_len.checked_sub(match_rng.1) {
                    diff <= tolerance
                } else {
                    panic!(
                        "Bug: range end greater than len ({} > {})",
                        match_rng.1, text_len
                    );
                }
            }
        }
    }
}

/// General options/properties derived from CLI args
#[derive(Debug)]
pub struct SearchOpts {
    pub in_order: bool,
    pub seqtype: SeqType,
    pub hit_scoring: HitScoring,
    pub case_insensitive: bool,
    pub attr: RecordAttr,
    pub replacement: Option<String>,
    pub threads: u32,
}

/// Options related to filtering
#[derive(Debug, Clone)]
pub struct FilterOpts {
    pub filter: Option<bool>,
    pub dropped_path: Option<String>,
}

/// Options on how much information is required
/// (derived from `SearchConfig` once configuration is finished)
#[derive(Debug, Clone)]
pub struct SearchRequirements {
    pub required_detail: RequiredDetail,
    pub max_hits: usize,
    pub has_regex_groups: bool,
}

/// Main configuration object holding the patterns, and search settings
#[derive(Debug, Default)]
pub struct SearchConfig {
    patterns: Vec<PatternConfig>,
    search_range: Option<Range>,
    anchor: Option<Anchor>,
    /// group numbers (0 = full match);
    /// may be empty if max_hits == 0 and required_info == RequiredInfo::Exists
    required_groups: Vec<usize>,
    /// group number -> index in groups vector;
    /// only defined if regex groups should be located (not just group 0 = full match)
    group_idx: VecMap<usize>,
    // overall required level of detail
    detail: RequiredDetail,
    /// maximum number of required hits
    /// 0 = RequiredInfo::Exsists only
    /// usize::MAX for all hits
    required_hits: usize,
}

impl SearchConfig {
    pub fn new(patterns: Vec<PatternConfig>) -> Self {
        Self {
            patterns,
            ..Default::default()
        }
    }

    pub fn patterns(&self) -> &[PatternConfig] {
        &self.patterns
    }

    pub fn set_search_range(&mut self, range: Range) {
        self.search_range = Some(range);
    }

    pub fn get_search_range(&self) -> Option<Range> {
        self.search_range
    }

    /// Sets the required level of detail
    fn set_detail(&mut self, detail: RequiredDetail) {
        self.detail = self.detail.max(detail);
    }

    pub fn get_required_detail(&self) -> RequiredDetail {
        self.detail
    }

    /// Sets the number of required hits
    pub fn require_n_hits(&mut self, max_hits: usize, detail: RequiredDetail) {
        self.required_hits = self.required_hits.max(max_hits);
        self.set_detail(detail);
        if detail >= RequiredDetail::Range {
            self.require_group(0);
        }
    }

    /// Returns the number of required hits (usize::MAX = all hits)
    pub fn get_required_hits(&self) -> usize {
        self.required_hits
    }

    /// Requires the search of a specific (regex) group
    /// (group 0 = full match)
    pub fn require_group(&mut self, group: usize) {
        // if group == 0 {
        //     if self.groups.is_empty() {
        //         self.groups.push(0);
        //     }
        //     assert!(&self.groups == &[0]);
        // } else {
        // let group_idx = self.group_idx.get_or_insert_with(VecMap::new);
        self.group_idx.entry(group).or_insert_with(|| {
            let l = self.required_groups.len();
            self.required_groups.push(group);
            l
        });
        // }
    }

    /// Returns a slice of all requested group numbers
    /// (full hit = 0, regex groups = 1..)
    pub fn get_required_groups(&self) -> &[usize] {
        &self.required_groups
    }

    /// Returns the index of the group in `self.groups`
    pub fn get_group_idx(&self, group: usize) -> Option<usize> {
        self.group_idx.get(group).cloned()
        // if let Some(group_idx) = &self.group_idx {
        //     group_idx.get(group).cloned()
        // } else if group == 0 {
        //     Some(0)
        // } else {
        //     None
        // }
    }

    /// Returns the group number corresponding to the group name,
    /// also verifying that all supplied patterns are consistent
    /// (no variable order of named groups)
    pub fn resolve_named_group(&self, group: &str) -> Result<usize, String> {
        let mut num = None;
        for p in &self.patterns {
            assert_eq!(p.algorithm, Algorithm::Regex);
            let _n = resolve_group(&p.pattern.seq, group)?;
            if let Some(n) = num {
                if n != _n {
                    return Err(format!(
                        "Named group '{}' does not resolve to the same group number in all patterns.\
                        This is a requirement in the case of multiple regex patterns. \
                        Consider using simple group numbers instead.",
                        group,
                    ));
                }
            } else {
                num = Some(_n);
            }
        }
        Ok(num.unwrap())
    }

    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.anchor = Some(anchor);
        self.require_group(0);
        self.require_n_hits(1, RequiredDetail::Range);
    }

    pub fn get_anchor(&self) -> Option<Anchor> {
        self.anchor
    }

    pub fn get_search_requirements(&self) -> SearchRequirements {
        SearchRequirements {
            required_detail: self.detail,
            max_hits: self.required_hits,
            has_regex_groups: self.required_groups.iter().any(|g| *g > 0), // self.group_idx.is_some(),
        }
    }

    pub fn init_matches(&self) -> Matches {
        Matches::new(self.patterns.len(), self.required_groups.len())
    }

    /// Returns hit no. `hit_i` (0-based index) of given group for `pattern_rank` best-matching pattern
    pub fn get_hit<'a>(
        &self,
        matches: &'a Matches,
        hit_i: usize,
        pattern_rank: usize,
        group: usize,
    ) -> Option<&'a Match> {
        let group_i = self.get_group_idx(group).unwrap();
        matches.get_hit(hit_i, pattern_rank, group_i)
    }

    /// Iterates across all hits of a specific group index
    pub fn hits_iter<'a>(
        &self,
        matches: &'a Matches,
        pattern_rank: usize,
        group: usize,
    ) -> impl Iterator<Item = &'a Match> {
        let group_i = self.get_group_idx(group).unwrap();
        matches.hits_iter(pattern_rank, group_i)
    }

    /// Returns the pattern with the given rank (given a `Matches` object) or `None`
    /// if the pattern was not found
    pub fn matched_pattern(
        &self,
        pattern_rank: usize,
        matches: &Matches,
    ) -> Option<&PatternConfig> {
        let pattern_idx = matches.get_pattern_idx(pattern_rank)?;
        self.patterns.get(pattern_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor() {
        assert!(Anchor::Start(1).is_anchored((1, 2), 10));
        assert!(!Anchor::Start(1).is_anchored((2, 3), 10));
        assert!(Anchor::End(0).is_anchored((5, 10), 10));
        assert!(Anchor::End(1).is_anchored((5, 9), 10));
        assert!(Anchor::End(2).is_anchored((5, 9), 10));
        assert_eq!(Anchor::Start(2).get_search_range(2, 10), (0, 4));
        assert_eq!(Anchor::Start(2).get_search_range(5, 10), (0, 7));
        assert_eq!(Anchor::Start(5).get_search_range(10, 10), (0, 10));
        assert_eq!(Anchor::End(0).get_search_range(3, 10), (7, 10));
        assert_eq!(Anchor::End(2).get_search_range(3, 10), (5, 10));
        assert_eq!(Anchor::End(9).get_search_range(3, 10), (0, 10));
    }
}
