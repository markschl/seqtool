use vec_map::VecMap;

use crate::helpers::rng::Range;

use super::matcher::{Match, Matcher};
use super::opts::Shift;

/// Holds information about matches found by `Matcher`, sent around between threads
/// in case of parallel searching.
#[derive(Debug)]
pub struct Matches {
    inner: MatchesInner,
    pattern_names: Vec<Option<String>>,
    patterns: Vec<String>,
    // search only within these bounds
    bounds: Option<Range>,
    // has_matches: bool,
}

impl Matches {
    pub fn new(
        pattern_names: Vec<Option<String>>,
        patterns: Vec<String>,
        groups: Vec<usize>,
        max_hits: usize,
        max_shift: Option<Shift>,
        bounds: Option<Range>,
    ) -> Self {
        Matches {
            inner: MatchesInner::new(pattern_names.len(), groups, max_hits, max_shift),
            pattern_names,
            patterns,
            bounds,
        }
    }

    pub fn find(
        &mut self,
        text: &[u8],
        matchers: &mut [Box<dyn Matcher + Send>],
    ) -> Result<(), String> {
        // restrict search range if necessary
        let (text, offset) = if let Some(bounds) = self.bounds {
            // restrict search range
            let (s, e) = bounds.obtain(text.len());
            (&text[s..e], s)
        } else {
            (text, 0)
        };
        // do the searching
        self.inner.collect(text, matchers, offset)
    }

    pub fn has_matches(&self) -> bool {
        self.inner.has_matches()
    }

    pub fn matches_iter(
        &self,
        pattern_rank: usize,
        group: usize,
    ) -> impl Iterator<Item = &'_ Match> {
        self.inner.matches_iter(pattern_rank, group)
    }

    pub fn get_match(&self, pos: usize, pattern_rank: usize, group: usize) -> Option<&Match> {
        self.inner.get(pos, pattern_rank, group)
    }

    pub fn pattern_name(&self, pattern_rank: usize) -> Option<Option<&str>> {
        self.pattern_names
            .get(self.inner.pattern_idx(pattern_rank))
            .map(|n| n.as_deref())
    }

    pub fn pattern(&self, pattern_rank: usize) -> Option<&str> {
        self.patterns
            .get(self.inner.pattern_idx(pattern_rank))
            .map(|p| p.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MatchesInner {
    /// max. shift at start/end
    max_shift: Option<Shift>,
    /// max. required number of hits
    /// (0 = check for presence only, no distance or range required)
    max_hits: usize,
    /// match storage for each pattern
    /// may not actually be used if search_limit == 0
    data: Vec<PatternMatches>,
    /// group numbers (0 = full hit);
    /// may be empty if search_limit == 0
    groups: Vec<usize>,
    /// group number -> index in groups vector (only defined if >1 groups)
    group_idx: Option<VecMap<usize>>,
    /// did the current search lead to any matches?
    has_matches: bool,
}

impl MatchesInner {
    pub fn new(
        n_patterns: usize,
        groups: Vec<usize>,
        search_limit: usize,
        max_shift: Option<Shift>,
    ) -> Self {
        let group_idx = if !groups.is_empty() {
            Some(groups.iter().enumerate().map(|(i, g)| (*g, i)).collect())
        } else {
            None
        };
        Self {
            max_shift,
            data: vec![PatternMatches::new(groups.len()); n_patterns],
            max_hits: search_limit,
            groups,
            group_idx,
            has_matches: false,
        }
    }

    /// Collects matches into the internal hit matrices, one per supplied pattern matcher.
    ///
    /// After searching, sorts the internal vector of matrices:
    /// - patterns with hits come first, patterns without hits are last
    ///   (but within both groups, patterns remain in original order)
    /// - patterns with hits are sorted by edit distance (in case it varies)
    fn collect(
        &mut self,
        text: &[u8],
        matchers: &mut [Box<dyn Matcher + Send>],
        offset: usize,
    ) -> Result<(), String> {
        // simple filtering: return true at first matching pattern
        if self.max_hits == 0 {
            for m in matchers {
                if m.has_matches(text)? {
                    self.has_matches = true;
                    return Ok(());
                }
            }
            self.has_matches = false;
            return Ok(());
        }

        // more information needed: collect hits
        self.has_matches = false;
        assert!(matchers.len() == self.data.len());
        for (i, matcher) in matchers.iter_mut().enumerate() {
            let data = &mut self.data[i];
            data.init(i);
            let mut num_found = 0;

            matcher.iter_matches(text, &mut |h| {
                // set matches
                let out = data.next_mut();
                assert!(self.groups.len() == out.len());
                for (group, m) in self.groups.iter().zip(out.iter_mut()) {
                    h.get_group(*group, m)?;
                    m.start += offset;
                    m.end += offset;
                }

                // check max-shift (assuming group 0 (= full hit) to be present)
                if let Some(s) = self.max_shift.as_ref() {
                    let full_pos = &out[0];
                    if !s.in_range((full_pos.start - offset, full_pos.end - offset), text.len()) {
                        data.clear_current();
                        return Ok(false);
                    }
                }

                // stop if hit limit reached
                num_found += 1;
                // dbg!(num_found, self.max_hits, &out);
                if num_found >= self.max_hits {
                    return Ok(false);
                }
                Ok(true)
            })?;

            if !data.is_empty() {
                self.has_matches = true;
            }
        }

        // then, sort by (<has hit>, edit distance)
        if self.has_matches && self.data.len() > 1 {
            self.data
                .sort_by_key(|d| (d.is_empty(), d.hit(0, 0).map(|m| m.dist).unwrap_or(0)));
        }
        // dbg!(&self.data);
        Ok(())
    }

    pub fn has_matches(&self) -> bool {
        self.has_matches
    }

    pub fn pattern_idx(&self, rank: usize) -> usize {
        if self.data.len() == 1 {
            0
        } else {
            self.data[rank].pattern_i
        }
    }

    /// Returns hit no. `hit_i` (0-based index) of given group for `pattern_rank` best-matching pattern
    pub fn get(&self, hit_i: usize, pattern_rank: usize, group: usize) -> Option<&Match> {
        let group_i = *self
            .group_idx
            .as_ref()
            .and_then(|i| i.get(group))
            .unwrap_or(&0);
        self.data[pattern_rank].hit(hit_i, group_i)
    }

    /// Iterates across all hits of a specific group
    fn matches_iter(&self, pattern_rank: usize, group: usize) -> impl Iterator<Item = &'_ Match> {
        let group_i = self.group_idx.as_ref().map(|i| i[group]).unwrap_or(0);
        self.data[pattern_rank].hits_iter(group_i)
    }
}

/// Holds sequence matches for a single pattern. Since multiple `PatternMatches` objects
/// are sorted, these objects do not necessarily always serve as storage for the same pattern,
/// they may switch between patterns.
#[derive(Debug, Clone, Default)]
struct PatternMatches {
    /// Number of (regex) groups; group = 0 is always the full hit, so there is
    /// always at least one group.
    num_groups: usize,
    /// flat 2D matrix
    /// N hits x M groups
    matches: Vec<Match>,
    /// The 'matches' vector is never truncated (only grown)
    /// in order to save the allocations of the 'operations' vector.
    /// This field holds current length of the flat matrix, independently of the
    /// actual vector length
    len: usize,
    /// current pattern number (0-based index)
    /// this must be known, since multiple `PatternMatches` will be sorted
    pattern_i: usize,
}

impl PatternMatches {
    fn new(num_groups: usize) -> Self {
        Self {
            num_groups,
            matches: Vec::new(),
            len: 0,
            pattern_i: 0,
        }
    }

    fn hits_iter(&self, group_i: usize) -> impl Iterator<Item = &'_ Match> {
        self.matches[..self.len]
            .iter()
            .skip(group_i)
            .step_by(self.num_groups)
    }

    pub fn hit(&self, hit_i: usize, group_i: usize) -> Option<&Match> {
        debug_assert!(group_i < self.num_groups);
        let i = hit_i * self.num_groups + group_i;
        if i < self.len {
            self.matches.get(i)
        } else {
            None
        }
    }

    /// Clears the storage for a new sequence search for pattern with given index
    fn init(&mut self, pattern_i: usize) {
        self.len = 0;
        self.pattern_i = pattern_i;
    }

    /// Returns a mutable reference to the next match
    fn next_mut(&mut self) -> &mut [Match] {
        let old_len = self.len;
        self.len += self.num_groups;
        if self.len > self.matches.len() {
            debug_assert!(self.len == self.matches.len() + self.num_groups);
            self.matches.resize(self.len, Default::default());
        }
        &mut self.matches[old_len..self.len]
    }

    /// Removes the data of the current hit (as obtained by `next_mut`), meaning
    /// that it is not a valid hit
    fn clear_current(&mut self) {
        self.len -= self.num_groups;
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}
