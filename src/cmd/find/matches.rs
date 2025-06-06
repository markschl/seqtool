use super::matcher::{Match, Matcher};
use super::opts::{Anchor, RequiredDetail, SearchConfig};

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
        matchers: &mut [Box<dyn Matcher + Send + Sync>],
        cfg: &SearchConfig,
    ) -> Result<(), String> {
        // restrict search range if necessary
        let (text_restricted, offset) = if let Some(rng) = cfg.get_search_range() {
            let (s, e) = rng.resolve(text.len());
            (&text[s..e], s)
        } else {
            (text, 0)
        };

        // simple filtering: return successfully at first match by any pattern
        if cfg.get_required_detail() == RequiredDetail::HasMatch {
            assert!(cfg.get_anchor().is_none()); // anchoring requires more elaborate strategy below
            for m in matchers {
                if m.has_matches(text_restricted)? {
                    self.has_matches = true;
                    return Ok(());
                }
            }
            self.has_matches = false;
            return Ok(());
        }

        // more information needed: collect hits into `self.matches`
        self.has_matches = false;
        assert!(matchers.len() == self.data.len());
        for ((pattern_i, matcher), (data_i, data)) in
            matchers.iter_mut().enumerate().zip(&mut self.data)
        {
            // assign pattern index
            *data_i = pattern_i;
            data.clear();

            // in case of anchoring, we can further restrict the search range
            let (text_restricted2, offset_adj) = if let Some(anchor) = cfg.get_anchor() {
                let pattern_cfg = &cfg.patterns()[pattern_i];
                // longest possible range covered by pattern
                let pattern_rng = pattern_cfg.pattern.seq.len() + pattern_cfg.max_dist;
                let (start, end) = anchor.get_search_range(pattern_rng, text_restricted.len());
                (&text_restricted[start..end], offset + start)
            } else {
                (text_restricted, offset)
            };

            matcher.do_search(text_restricted2, &mut |hit| {
                // set matches
                let out = data.next_mut();
                assert!(cfg.get_required_groups().len() == out.len());
                for (group, match_) in cfg.get_required_groups().iter().zip(out.iter_mut()) {
                    hit.get_group(*group, match_)?;
                }

                // stop if hit limit reached
                if data.num_hits() >= cfg.get_hit_limit() {
                    return Ok(false);
                }
                Ok(true)
            })?;

            if let Some(anchor) = cfg.get_anchor() {
                // we assume that a full match (group 0) with RequiredDetail::Range has been registered
                let match_group_i = cfg.get_group_idx(0).unwrap();
                if data.apply_anchor(anchor, match_group_i, text_restricted2.len()) {
                    self.has_matches = true;
                }
            } else if !data.is_empty() {
                self.has_matches = true;
            }

            // adjust coordinates by introduced offset
            if offset_adj != 0 {
                for m in data.matches_slice_raw_mut() {
                    m.start += offset_adj;
                    m.end += offset_adj;
                }
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
    pub fn get_hit(&self, hit_i: isize, pattern_rank: usize, group_i: usize) -> Option<&Match> {
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

/// Holds all matches for a single pattern in a given text.
/// In case of multiple patterns, `PatternMatches` objects may be sorted,
/// and therefore not always correspond to the same pattern.
#[derive(Debug, Clone, Default)]
struct PatternMatches {
    /// Flat matrix of N hits x M groups, matches stored in row-major order
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

    /// Returns the flat matrix
    fn matches_slice_raw(&self) -> &[Match] {
        &self.matches[0..self.num_hits * self.num_groups]
    }

    /// Returns the flat matrix (mutable)
    fn matches_slice_raw_mut(&mut self) -> &mut [Match] {
        &mut self.matches[0..self.num_hits * self.num_groups]
    }

    /// Returns an iterator over all hits of a specific group index
    /// (not always the same as group number, see `SearchConfig::get_group_idx()`)
    fn hits_iter(&self, group_i: usize) -> impl Iterator<Item = &'_ Match> {
        self.matches_slice_raw()
            .iter()
            .skip(group_i)
            .step_by(self.num_groups)
            .take(self.num_hits)
    }

    /// Returns a normalized non-negative hit index, or `None` if the requested
    /// index is out of bounds
    pub fn _normalized_hit_i(&self, hit_i: isize) -> Option<usize> {
        if hit_i >= 0 {
            let hit_i = hit_i as usize;
            if hit_i < self.num_hits {
                return Some(hit_i);
            }
        } else if let Some(i) = self.num_hits.checked_sub((-hit_i) as usize) {
            return Some(i);
        }
        None
    }

    /// Returns a `Match` corresponding to the given hit and group index
    pub fn get_hit(&self, hit_i: isize, group_i: usize) -> Option<&Match> {
        self._normalized_hit_i(hit_i).and_then(|_hit_i| {
            debug_assert!(group_i < self.num_groups);
            let i = _hit_i * self.num_groups + group_i;
            self.matches.get(i)
        })
    }

    /// Clears the storage for a new sequence search
    fn clear(&mut self) {
        self.num_hits = 0;
    }

    /// Returns a mutable reference to the next match
    /// (a slice of `self.num_groups` `Match` objects),
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

    /// Selects a single anchored hit (first or last depending on the anchor type)
    /// among the current hits, and discards the rest.
    /// Returns `true` if there is an anchored match.
    fn apply_anchor(&mut self, anchor: Anchor, group_i: usize, text_len: usize) -> bool {
        // Obtain the index of the first or last anchored hit depending on the anchor type.
        // Due to the limited search range, usually there is only one hit and hit_i will be
        // `Some(0)` or `None`.
        let hit_i = {
            let mut anchored_iter = self.hits_iter(group_i).enumerate().filter_map(|(i, pos)| {
                if anchor.is_anchored((pos.start, pos.end), text_len) {
                    Some(i)
                } else {
                    None
                }
            });
            match anchor {
                Anchor::Start(_) => anchored_iter.nth(0),
                Anchor::End(_) => anchored_iter.last(),
            }
        };
        // If there is an anchored hit chosen among multiple hits, move it to the front
        // and make it the only hit
        if let Some(i) = hit_i {
            if i != 0 {
                let (hit0, rest) = self.matches.split_at_mut(self.num_groups);
                let rest_i = (i - 1) * self.num_groups;
                hit0.clone_from_slice(&rest[rest_i..rest_i + self.num_groups]);
            }
            self.num_hits = 1;
        } else {
            self.num_hits = 0;
        }
        self.num_hits > 0
    }

    fn num_hits(&self) -> usize {
        self.num_hits
    }

    fn is_empty(&self) -> bool {
        self.num_hits == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matches() {
        macro_rules! Match {
            ($start:expr, $end:expr) => {
                Match {
                    start: $start,
                    end: $end,
                    dist: 0,
                    alignment_path: Vec::new(),
                }
            };
        }
        let mut matches = PatternMatches::new(2);
        assert!(matches.get_hit(0, 0).is_none());
        let group_m = matches.next_mut();
        group_m[0] = Match!(5, 10);
        group_m[1] = Match!(5, 8);
        let group_m = matches.next_mut();
        group_m[0] = Match!(10, 15);
        group_m[1] = Match!(10, 13);
        let group_m = matches.next_mut();
        group_m[0] = Match!(1, 5);
        group_m[1] = Match!(1, 3);

        assert_eq!(matches.get_hit(0, 0).unwrap().end, 10);
        assert_eq!(matches.get_hit(1, 0).unwrap().end, 15);
        assert_eq!(matches.get_hit(2, 0).unwrap().end, 5);
        assert_eq!(matches.get_hit(1, 1).unwrap().end, 13);

        let m: Vec<_> = matches.hits_iter(0).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, vec![(5, 10), (10, 15), (1, 5)]);
        let m: Vec<_> = matches.hits_iter(1).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, vec![(5, 8), (10, 13), (1, 3)]);

        let mut anchored_m = matches.clone();
        assert!(!anchored_m.apply_anchor(Anchor::Start(0), 0, 16));
        assert!(anchored_m.hits_iter(0).next().is_none());

        let mut anchored_m = matches.clone();
        assert!(anchored_m.apply_anchor(Anchor::Start(1), 0, 16));
        let m: Vec<_> = anchored_m.hits_iter(0).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, &[(1, 5)]);

        let mut anchored_m = matches.clone();
        assert!(anchored_m.apply_anchor(Anchor::Start(5), 0, 16));
        let m: Vec<_> = anchored_m.hits_iter(0).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, &[(5, 10)]);

        let mut anchored_m = matches.clone();
        assert!(!anchored_m.apply_anchor(Anchor::End(0), 0, 16));
        assert!(anchored_m.hits_iter(0).next().is_none());

        let mut anchored_m = matches.clone();
        assert!(anchored_m.apply_anchor(Anchor::End(1), 0, 16));
        let m: Vec<_> = anchored_m.hits_iter(0).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, &[(10, 15)]);

        let mut anchored_m = matches.clone();
        assert!(anchored_m.apply_anchor(Anchor::End(11), 0, 16));
        let m: Vec<_> = anchored_m.hits_iter(0).map(|m| (m.start, m.end)).collect();
        assert_eq!(m, &[(1, 5)]);
    }
}
