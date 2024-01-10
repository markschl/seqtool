use std::iter::{Skip, StepBy};
use std::slice::Iter;

use vec_map::VecMap;

use crate::helpers::rng::Range;

use super::matcher::{Match, Matcher};
use super::*;

/// Sent around between threads and holds the matches found by `Matcher`
#[derive(Debug)]
pub struct Matches {
    pattern_names: Vec<String>,
    cfg: SearchConfig,
    max_shift: Option<Shift>,
    multiple_patterns: bool,
    bounds: Option<Range>,
    calc_bounds: Option<(usize, usize)>,
    // offset introduced by narrowing down search range (calc_bounds)
    offset: usize,
    // vector of matches for each pattern
    // Vec<Option<Match>> is a flat 2D matrix with dimension
    // num_patterns x num_match_groups
    match_vecs: Vec<Vec<Option<Match>>>,
    has_matches: bool,
    // dist, index, has_matches
    dist_order: Vec<(u16, usize, bool)>,
    // optional search range
}

impl Matches {
    pub fn new(
        pattern_names: &[String],
        pos: SearchConfig,
        bounds: Option<Range>,
        max_shift: Option<Shift>,
    ) -> Matches {
        let n_patterns = pattern_names.len();
        Matches {
            pattern_names: pattern_names.to_owned(),
            cfg: pos,
            max_shift,
            multiple_patterns: n_patterns > 1,
            dist_order: vec![(0, 0, false); n_patterns],
            match_vecs: vec![vec![]; n_patterns],
            has_matches: false,
            bounds,
            calc_bounds: None,
            offset: 0,
        }
    }

    pub fn find<M: Matcher>(&mut self, text: &[u8], matchers: &mut [M]) -> CliResult<()> {
        let mut text = text;

        if let Some(bounds) = self.bounds {
            // restrict search range
            let (s, e) = bounds.obtain(text.len());
            self.calc_bounds = Some((s, e));
            self.offset = s;
            text = &text[self.offset..e];
        }

        if !self.multiple_patterns {
            self.has_matches = self.cfg.collect_matches(
                text,
                &mut matchers[0],
                &mut self.match_vecs[0],
                self.max_shift.as_ref(),
                self.offset,
            )?;
        } else {
            for (i, ((matcher, matches), (best_dist, idx, has_matches))) in matchers
                .iter_mut()
                .zip(self.match_vecs.iter_mut())
                .zip(self.dist_order.iter_mut())
                .enumerate()
            {
                *has_matches = self.cfg.collect_matches(
                    text,
                    matcher,
                    matches,
                    self.max_shift.as_ref(),
                    self.offset,
                )?;
                // take distance of first match (assumed to be sorted if necessary)
                *best_dist = matches
                    .get(0)
                    .and_then(|m| m.as_ref().map(|m| m.dist))
                    .unwrap_or(std::u16::MAX);
                *idx = i;
            }
            // sort -> best matches first
            self.dist_order.sort_by_key(|&(dist, _, _)| dist);
            self.has_matches = self.dist_order[0].2;
        }
        Ok(())
    }

    pub fn has_matches(&self) -> bool {
        self.has_matches
    }

    fn _matches(&self, pattern_rank: usize) -> &[Option<Match>] {
        if self.multiple_patterns {
            &self.match_vecs[self.dist_order[pattern_rank].1]
        } else {
            &self.match_vecs[pattern_rank]
        }
    }

    pub fn matches_iter(&self, pattern_rank: usize, group: usize) -> MatchesIter {
        self.cfg.iter(group, self._matches(pattern_rank))
    }

    pub fn get_match(&self, pos: usize, group: usize, pattern_rank: usize) -> Option<&Match> {
        self.cfg.get(pos, group, self._matches(pattern_rank))
    }

    pub fn pattern_name(&self, pattern_rank: usize) -> Option<&str> {
        if self.multiple_patterns {
            return self
                .dist_order
                .get(pattern_rank)
                .and_then(|&(_, i, has_matches)| {
                    if has_matches {
                        return self.pattern_names.get(i).map(String::as_str);
                    }
                    None
                });
        } else if self.has_matches {
            return Some(&self.pattern_names[0]);
        }
        None
    }
}

/// `SearchConfig` holds the information about which match indices / match groups are requested.
/// Additionally, it knows how to fill a `Vec<Option<Match>>` (which is a flat 2D matrix)
/// and provides methods for accessing it.
/// TODO: This is not optimal as the vector is not owned by `SearchConfig`.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    search_limit: usize,
    // group numbers
    groups: Vec<usize>,
    // group number -> index in groups vector
    group_idx: VecMap<usize>,
}

impl SearchConfig {
    pub fn new() -> SearchConfig {
        SearchConfig {
            search_limit: 0,
            groups: vec![],
            group_idx: VecMap::new(),
        }
    }

    fn add_group(&mut self, group: usize) {
        let groups = &mut self.groups;
        self.group_idx.entry(group).or_insert_with(|| {
            let l = groups.len();
            groups.push(group);
            l
        });
    }

    pub fn register_pos(&mut self, pos: usize, group: usize) {
        if pos >= self.search_limit {
            self.search_limit = pos + 1;
        }
        self.add_group(group);
    }

    pub fn register_all(&mut self, group: usize) {
        self.search_limit = std::usize::MAX;
        self.add_group(group);
    }

    pub fn has_groups(&self) -> bool {
        self.groups.iter().any(|g| *g > 0)
    }

    // collects matches into 'matches' which is a flat 2D matrix
    fn collect_matches<M>(
        &self,
        text: &[u8],
        mut matcher: M,
        matches: &mut Vec<Option<Match>>,
        max_shift: Option<&Shift>,
        offset: usize,
    ) -> CliResult<bool>
    where
        M: Matcher,
    {
        matches.clear();
        let mut num_found = 0;

        matcher.iter_matches(text, &mut |h| {
            let (start, end) = h.pos();

            // pre-filter
            if let Some(s) = max_shift.as_ref() {
                if !s.in_range((start, end), text.len()) {
                    for _ in 0..self.groups.len() {
                        matches.push(None);
                    }
                    return true;
                }
            }

            num_found += 1;

            if self.search_limit == 0 {
                return false;
            }

            for group in &self.groups {
                let m = h.group(*group).map(|mut m| {
                    m.start += offset;
                    m.end += offset;
                    m
                });
                matches.push(m);
            }

            if num_found >= self.search_limit {
                return false;
            }

            true
        })?;
        Ok(num_found > 0)
    }

    fn iter<'a>(&self, group: usize, matches: &'a [Option<Match>]) -> MatchesIter<'a> {
        let group_idx = self.group_idx[group];
        MatchesIter {
            matches: matches.iter().skip(group_idx).step_by(self.groups.len()),
        }
    }

    pub fn get<'a>(
        &self,
        pos: usize,
        group: usize,
        matches: &'a [Option<Match>],
    ) -> Option<&'a Match> {
        let i = pos * self.groups.len() + self.group_idx[group];
        matches.get(i).and_then(|m| m.as_ref())
    }
}

pub struct MatchesIter<'a> {
    matches: StepBy<Skip<Iter<'a, Option<Match>>>>,
}

impl<'a> Iterator for MatchesIter<'a> {
    type Item = Option<&'a Match>;
    fn next(&mut self) -> Option<Self::Item> {
        self.matches.next().map(|m| m.as_ref())
    }
}

#[derive(Debug, Clone)]
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
