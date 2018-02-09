use std::iter::Skip;
use std::slice::Iter;

use vec_map::VecMap;
use itertools::{Itertools, Step};

use super::matcher::{Match, Matcher};
use super::*;

// Sent around between threads and holds the matches found by `Matcher`
#[derive(Debug)]
pub struct Matches {
    matcher_names: Vec<String>,
    pos: SearchPositions,
    bounds: Option<(isize, isize)>,
    max_shift: Option<Shift>,
    multiple_matchers: bool,
    calc_bounds: Option<(usize, usize)>,
    // offset introduced by narrowing down search range (calc_bounds)
    offset: usize,
    // vector of matches for each pattern
    // Vec<Option<Match>> is a flat 2D matrix with dimension has_matches x num_match_groups
    matches: Vec<Vec<Option<Match>>>,
    has_matches: bool,
    // dist, index, has_matches
    dist_order: Vec<(u16, usize, bool)>,
    // optional search range
}

impl Matches {
    pub fn new(
        matcher_names: &[String],
        pos: SearchPositions,
        bounds: Option<(isize, isize)>,
        max_shift: Option<Shift>,
    ) -> Matches {
        let n = matcher_names.len();
        Matches {
            matcher_names: matcher_names.to_owned(),
            pos: pos,
            max_shift: max_shift,
            multiple_matchers: n > 1,
            dist_order: vec![(0, 0, false); n],
            matches: vec![vec![]; n],
            has_matches: false,
            bounds: bounds,
            calc_bounds: None,
            offset: 0,
        }
    }

    pub fn find<M: Matcher>(&mut self, text: &[u8], matchers: &mut [M]) {
        let mut text = text;

        if let Some((start, end)) = self.bounds {
            // restrict search range
            let (s, e) = Range::from_rng1(start, end, text.len()).get(false);
            self.calc_bounds = Some((s, e));
            self.offset = s;
            text = &text[self.offset..e];
        }

        if !self.multiple_matchers {
            self.has_matches = self.pos.collect_matches(
                text,
                &mut matchers[0],
                &mut self.matches[0],
                self.max_shift.as_ref(),
                self.offset,
            );
        } else {
            for (i, ((ref mut matcher, ref mut matches),
                    &mut (ref mut best_dist, ref mut idx, ref mut has_matches))) in matchers
                        .into_iter()
                        .zip(self.matches.iter_mut())
                        .zip(self.dist_order.iter_mut())
                        .enumerate() {
                *has_matches = self.pos.collect_matches(
                    text,
                    matcher,
                    matches,
                    self.max_shift.as_ref(),
                    self.offset,
                );
                // take distance of first match (assumed to be sorted if necessary)
                *best_dist = matches
                    .get(0)
                    .and_then(|m| m.as_ref().map(|m| m.dist))
                    .unwrap_or(::std::u16::MAX);
                *idx = i;
            }
            // sort -> best matches first
            self.dist_order.sort_by_key(|&(dist, _, _)| dist);
            self.has_matches = self.dist_order[0].2;
        }
    }

    pub fn has_matches(&self) -> bool {
        self.has_matches
    }

    fn _matches(&self, pattern_rank: usize) -> &[Option<Match>] {
        if self.multiple_matchers {
            &self.matches[self.dist_order[pattern_rank].1]
        } else {
            &self.matches[pattern_rank]
        }
    }

    pub fn matches_iter(&self, pattern_rank: usize, group: usize) -> MatchesIter {
        self.pos.matches_iter(group, self._matches(pattern_rank))
    }

    pub fn get_match(&self, pos: usize, group: usize, pattern_rank: usize) -> Option<&Match> {
        self.pos.get_match(pos, group, self._matches(pattern_rank))
    }

    pub fn pattern_name(&self, pattern_rank: usize) -> Option<&str> {
        if self.multiple_matchers {
            return self.dist_order
                .get(pattern_rank)
                .and_then(|&(_, i, has_matches)| {
                    if has_matches {
                        return self.matcher_names.get(i).map(String::as_str);
                    }
                    None
                });
        } else if self.has_matches {
            return Some(&self.matcher_names[0]);
        }
        None
    }
}

/// `SearchPositions` holds the information about which match indices / match groups are requested.
/// Additionally, it knows how to fill a `Vec<Option<Match>>` (which is a flat 2D matrix)
/// and provides methods for accessing it.
/// TODO: This is not optimal as the vector is not owned by `SearchPositions` and does not have a
/// special type.
#[derive(Debug, Clone)]
pub struct SearchPositions {
    search_limit: usize,
    // group numbers
    groups: Vec<usize>,
    // group number -> index in groups vector
    group_idx: VecMap<usize>,
}

impl SearchPositions {
    pub fn new() -> SearchPositions {
        SearchPositions {
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

    // None means that all matches should be searched
    pub fn register_pos(&mut self, pos: usize, group: usize) {
        if pos >= self.search_limit {
            self.search_limit = pos + 1;
        }
        self.add_group(group);
    }

    // None means that all matches should be searched
    pub fn register_all(&mut self, group: usize) {
        self.search_limit = ::std::usize::MAX;
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
    ) -> bool
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
                let m = h.group(*group).and_then(|mut m| {
                    m.start += offset;
                    m.end += offset;
                    Some(m)
                });
                matches.push(m);
            }

            if num_found >= self.search_limit {
                return false;
            }

            true
        });
        num_found > 0
    }

    fn matches_iter<'a>(&self, group: usize, matches: &'a [Option<Match>]) -> MatchesIter<'a> {
        let group_idx = self.group_idx[group];
        MatchesIter {
            matches: matches.iter().skip(group_idx).step(self.groups.len()),
        }
    }

    pub fn get_match<'a>(
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
    matches: Step<Skip<Iter<'a, Option<Match>>>>,
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
            Shift::End(n) => if let Some(diff) = len.checked_sub(rng.1) {
                diff <= n
            } else {
                panic!(format!("Range end greater than len ({} > {})", rng.1, len));
            },
        }
    }
}
