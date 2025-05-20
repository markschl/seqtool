use super::matcher::{Match, Matcher};
use super::opts::{RequiredDetail, SearchConfig};

pub fn do_search(
    text: &[u8],
    matchers: &mut [Box<dyn Matcher + Send>],
    cfg: &SearchConfig,
    out: &mut Matches,
) -> Result<(), String> {
    // restrict search range if necessary
    let (text, offset) = if let Some(bounds) = cfg.get_search_range() {
        let (s, e) = bounds.obtain(text.len());
        (&text[s..e], s)
    } else {
        (text, 0)
    };
    // do the searching
    out.collect_hits(text, matchers, cfg, offset)
}

/// Holds information about the hits found by `Matcher`.
/// *Each* sequence record has a `Matches` object associated with it, which
/// cycles between the workers and the main thread in case of parallel searching.
#[derive(Debug, Clone, Default)]
pub struct Matches {
    /// Tuple of (pattern index, match storage) for each pattern;
    /// the pattern index is needed to keep track of the patterns after sorting.
    /// May not actually be used if max_hits == 0
    data: Vec<(usize, PatternMatches)>,
    /// did the current search lead to any matches?
    has_matches: bool,
}

impl Matches {
    /// Creates `Matches` with `n_patterns` internal `PatternMatches` objects,
    /// which are in turn growable 2D arrays of `n_groups` x `n_hits`
    pub fn new(n_patterns: usize, n_groups: usize) -> Self {
        Self {
            data: vec![(0, PatternMatches::new(n_groups)); n_patterns],
            has_matches: false,
        }
    }

    /// Collects matches into the internal hit matrices, one per supplied pattern matcher.
    ///
    /// After searching, sorts the internal vector of matrices:
    /// - patterns with hits come first, patterns without hits are last
    ///   (but within both groups, patterns remain in original order)
    /// - patterns with hits are sorted by edit distance (in case > 0)
    pub fn collect_hits(
        &mut self,
        text: &[u8],
        matchers: &mut [Box<dyn Matcher + Send>],
        cfg: &SearchConfig,
        seq_offset: usize,
    ) -> Result<(), String> {
        // simple filtering: return successfully at first match by any pattern
        if cfg.get_required_detail() == RequiredDetail::Exists {
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
        for ((pattern_i, matcher), (data_i, data)) in
            matchers.iter_mut().enumerate().zip(&mut self.data)
        {
            // assign pattern index
            *data_i = pattern_i;
            data.clear();
            let mut num_found = 0;

            matcher.do_search(text, &mut |h| {
                // set matches
                let out = data.next_mut();
                assert!(cfg.get_required_groups().len() == out.len());
                for (group, m) in cfg.get_required_groups().iter().zip(out.iter_mut()) {
                    h.get_group(*group, m)?;
                    m.start += seq_offset;
                    m.end += seq_offset;
                }

                // check anchoring (assuming group 0 (= full hit) to be present)
                if let Some(a) = cfg.get_anchor() {
                    let full_pos = &out[0];
                    if !a.in_range(
                        (full_pos.start - seq_offset, full_pos.end - seq_offset),
                        text.len(),
                    ) {
                        data.step_back();
                        return Ok(false);
                    }
                }

                // stop if hit limit reached
                num_found += 1;
                // dbg!(num_found, self.max_hits, &out);
                if num_found >= cfg.get_required_hits() {
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
                .sort_by_key(|(_, d)| (d.is_empty(), d.get_hit(0, 0).map(|m| m.dist).unwrap_or(0)));
            assert!(!self.data[0].1.is_empty());
        }
        Ok(())
    }

    pub fn has_matches(&self) -> bool {
        self.has_matches
    }

    /// Returns pattern index (0-based) of the pattern with the given rank
    pub fn get_pattern_idx(&self, pattern_rank: usize) -> Option<usize> {
        self.data
            .get(pattern_rank)
            .and_then(|(i, p)| if !p.is_empty() { Some(*i) } else { None })
    }

    /// Returns hit no. `hit_i` (0-based index) of given group index for `pattern_rank` best-matching pattern
    pub fn get_hit(&self, hit_i: usize, pattern_rank: usize, group_i: usize) -> Option<&Match> {
        self.data[pattern_rank].1.get_hit(hit_i, group_i)
    }

    /// Iterates across all hits of a specific group index
    pub fn hits_iter(
        &self,
        pattern_rank: usize,
        group_i: usize,
    ) -> impl Iterator<Item = &'_ Match> {
        self.data[pattern_rank].1.hits_iter(group_i)
    }
}

/// Holds sequence matches for a single pattern.
/// Since multiple `PatternMatches` objects may be sorted, these objects
/// do not necessarily always serve as storage for the same pattern.
/// (`clear()` is called on every new search)
#[derive(Debug, Clone, Default)]
struct PatternMatches {
    /// flat 2D matrix
    /// N hits x M groups
    matches: Vec<Match>,
    /// Number of currently stored hits (everything beyond is old data).
    /// The 'matches' vector is never truncated (only grown)
    /// in order to save the allocations of 'Match::alignment_path'.
    num_hits: usize,
    /// Number of (regex) groups; group = 0 is always the full hit, so there is
    /// always at least one group.
    num_groups: usize,
}

impl PatternMatches {
    fn new(num_groups: usize) -> Self {
        Self {
            matches: Vec::new(),
            num_hits: 0,
            num_groups,
        }
    }

    fn hits_iter(&self, group_i: usize) -> impl Iterator<Item = &'_ Match> {
        self.matches
            .iter()
            .skip(group_i)
            .step_by(self.num_groups)
            .take(self.num_hits)
    }

    pub fn get_hit(&self, hit_i: usize, group_i: usize) -> Option<&Match> {
        debug_assert!(group_i < self.num_groups);
        if hit_i < self.num_hits {
            let i = hit_i * self.num_groups + group_i;
            self.matches.get(i)
        } else {
            None
        }
    }

    /// Clears the storage for a new sequence search
    fn clear(&mut self) {
        self.num_hits = 0;
    }

    /// Returns a mutable reference to the next match,
    /// advancing `self.num_hits` by one
    fn next_mut(&mut self) -> &mut [Match] {
        let old_len = self.num_hits * self.num_groups;
        let new_len = old_len + self.num_groups;
        self.num_hits += 1;
        if new_len > self.matches.len() {
            debug_assert_eq!(self.matches.len(), old_len);
            self.matches.resize(new_len, Default::default());
        }
        &mut self.matches[old_len..new_len]
    }

    /// Moves back by one hit, reversing the effect of `next_mut()`
    fn step_back(&mut self) {
        self.num_hits -= 1;
    }

    fn is_empty(&self) -> bool {
        self.num_hits == 0
    }
}
