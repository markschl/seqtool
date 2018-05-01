// Copyright 2014-2016 Johannes Köster.
// Licensed under the MIT license (http://opensource.org/licenses/MIT)
// This file may not be copied, modified, or distributed
// except according to those terms.

// Modifications by M. Schlegel

//! Myers bit-parallel approximate pattern matching algorithm.
//! Finds all matches up to a given edit distance. The pattern has to fit into a bitvector,
//! and is here limited to 64 symbols.
//! Complexity: O(n)
//!
//! # Example
//!
//! ```
//! use pattern_matching::myers::Myers;
//!
//! # fn main() {
//! let text = b"ACCGTGGATGAGCGCCATAG";
//! let pattern =      b"TGAGCGT";
//!
//! let mut myers = Myers::new(pattern);
//! let occ: Vec<_> = myers.find_all_end(text, 1).collect();
//!
//! assert_eq!(occ, [(13, 1), (14, 1)]);
//! # }
//! ```


use std::iter;
use std::ops::Range;
use std::u64;
use std::default::Default;

use super::*;


/// Myers algorithm.
pub struct Myers {
    peq: [u64; 256],
    bound: u64,
    m: u8,
    tb: Traceback,
}


impl Myers {
    /// Create a new instance of Myers algorithm for a given pattern.
    pub fn new<'a, P: IntoTextIterator<'a>>(pattern: P) -> Self {
        Self::from_variants(pattern.into_iter().cloned().map(Some))
    }

    /// Like `Myers::new()`, but additionally allows for specifying
    /// multiple matching characters in a pattern.
    ///
    /// Example:
    ///
    /// ```
    /// use pattern_matching::myers::Myers;
    ///
    /// # fn main() {
    /// let text =    b"TGACGNTGA";
    /// let pattern = b"TGANGCTGA";
    ///
    /// // 'N' has no special meaning:
    /// let myers = Myers::new(pattern);
    /// assert_eq!(myers.distance(text), 2);
    ///
    /// // 'N' in a pattern matches all four bases, but 'N' in
    /// // the text does not match any (asymmetric matching):
    /// let myers_ambig_asymm = Myers::from_variants(pattern.into_iter().map(|&b| {
    ///     // replacing N with all possible bases. To replace vec![] with slices,
    ///     // the `ref_slice` crate may be a good solution.
    ///     if b == b'N' { b"ACGT".to_vec() } else { vec![b] }
    /// }));
    /// assert_eq!(myers_ambig_asymm.distance(text), 1);
    ///
    /// // 'N' matches both ways:
    /// let myers_ambig_symm = Myers::from_variants(pattern.into_iter().map(|&b| {
    ///     if b == b'N' { b"ACGT".to_vec() } else { vec![b, b'N'] }
    /// }));
    /// assert_eq!(myers_ambig_symm.distance(text), 0);
    /// # }
    pub fn from_variants<P, I>(pattern: P) -> Self
        where P: IntoIterator<Item=I>,
              I: IntoIterator<Item=u8>
    {
        let mut peq = [0; 256];
        let mut m = 0;
        for (i, var) in pattern.into_iter().enumerate() {
            m += 1;
            for a in var.into_iter() {
                peq[a as usize] |= 1 << i;
            }
        }

        assert!(m <= 64 && m > 0);

        Myers {
            peq: peq,
            bound: 1 << (m - 1),
            m: m as u8,
            tb: Traceback::new(),
        }
    }

    /// Create a new instance of Myers algorithm for a given pattern and a wildcard character
    /// that shall match any character.
    pub fn with_wildcard(pattern: TextSlice, wildcard: u8) -> Self {
        let mut myers = Self::new(pattern);
        // wildcard matches all symbols of the pattern.
        myers.peq[wildcard as usize] = u64::MAX;

        myers
    }

    fn step(&self, state: &mut State, a: u8) {
        let eq = self.peq[a as usize];
        let xv = eq | state.mv;
        let xh = ((eq & state.pv).wrapping_add(state.pv) ^ state.pv) | eq;

        let mut ph = state.mv | !(xh | state.pv);
        let mut mh = state.pv & xh;

        if ph & self.bound > 0 {
            state.dist += 1;
        } else if mh & self.bound > 0 {
            state.dist -= 1;
        }
        // state.dist += (ph & self.bound > 0) as u8;
        // state.dist -= (mh & self.bound > 0) as u8;
        // state.dist += ((ph & self.bound) >> (self.m - 1)) as u8;
        // state.dist -= ((mh & self.bound) >> (self.m - 1)) as u8;

        ph <<= 1;
        mh <<= 1;
        state.pv = mh | !(xv | ph);
        state.mv = ph & xv;
    }

    fn step_trace(&mut self, state: &mut State, a: u8) {
        self.step(state, a);
        self.tb.add_state(state.clone());
    }

    /// Calculate the global distance of the pattern to the given text.
    pub fn distance<'a, I: IntoTextIterator<'a>>(&self, text: I) -> u8 {
        let mut state = State::new(self.m);
        for &a in text {
            self.step(&mut state, a);
        }
        state.dist
    }

    /// Find all matches of pattern in the given text up to a given maximum distance.
    /// Matches are returned as an iterator over pairs of end position and distance.
    pub fn find_all_end<'a, I: IntoTextIterator<'a>>(&'a self,
                                                     text: I,
                                                     max_dist: u8)
                                                     -> Matches<I::IntoIter> {
        let state = State::new(self.m);
        Matches {
            myers: self,
            state: state,
            text: text.into_iter().enumerate(),
            max_dist: max_dist,
        }
    }

    /// Find all matches of pattern in the given text up to a given maximum distance.
    /// In contrast to `find_all_end`, matches are returned as an iterator over ranges
    /// of (start, end, distance). Note that the end coordinate is a range coordinate,
    /// not included in the range (end index + 1) and is thus not equivalent to the end
    /// position returned by `find_all_end()`. In order to obtain an alignment, use
    ///
    ///
    /// Example:
    ///
    /// ```
    /// use pattern_matching::myers::Myers;
    /// use pattern_matching::myers::AlignmentOperation::*;
    ///
    /// # fn main() {
    /// let text = b"ACCGTGGATGAGCGCCATAG";
    /// let pattern =      b"TGAGCGT";
    ///
    /// // only range coordinates required
    /// let mut myers = Myers::new(pattern);
    /// let occ: Vec<_> = myers.find_all_pos(text, 1).collect();
    /// assert_eq!(occ, [(8, 14, 1), (8, 15, 1)]);
    ///
    /// let mut myers = Myers::new(pattern);
    /// let mut aln = vec![];
    /// let mut matches = myers.find_all_pos(text, 1);
    /// assert_eq!(matches.next_path(&mut aln).unwrap(), (8, 14, 1));
    /// assert_eq!(aln, &[Match, Match, Match, Match, Match, Match, Ins]);
    /// assert_eq!(matches.next_path(&mut aln).unwrap(), (8, 15, 1));
    /// assert_eq!(aln, &[Match, Match, Match, Match, Match, Match, Subst]);
    /// # }
    pub fn find_all_pos<'a, I: IntoTextIterator<'a>>(&'a mut self,
                                                     text: I,
                                                     max_dist: u8)
                                                     -> FullMatches<I::IntoIter> {
        self.tb.init(self.m, max_dist as usize);
        let m = self.m;
        FullMatches {
            myers: self,
            state: State::new(m),
            text: text.into_iter().enumerate(),
            max_dist: max_dist,
        }
    }
}


/// The current algorithm state.
#[derive(Clone, Debug, Default)]
struct State {
    pv: u64,
    mv: u64,
    dist: u8,
}


impl State {
    /// Create new state.
    pub fn new(m: u8) -> Self {
        State {
            pv: (1 << m) - 1,
            mv: 0,
            dist: m,
        }
    }
}


/// Iterator over pairs of end positions and distance of matches.
pub struct Matches<'a, I: TextIterator<'a>> {
    myers: &'a Myers,
    state: State,
    text: iter::Enumerate<I>,
    max_dist: u8,
}

impl<'a, I: Iterator<Item = &'a u8>> Iterator for Matches<'a, I> {
    type Item = (usize, u8);

    fn next(&mut self) -> Option<(usize, u8)> {
        for (i, &a) in self.text.by_ref() {
            self.myers.step(&mut self.state, a);
            if self.state.dist <= self.max_dist {
                return Some((i, self.state.dist));
            }
        }
        None
    }
}

/// Iterator over pairs of end positions and distance of matches.
pub struct FullMatches<'a, I: TextIterator<'a>> {
    myers: &'a mut Myers,
    state: State,
    text: iter::Enumerate<I>,
    max_dist: u8,
}

impl<'a, I: TextIterator<'a>> FullMatches<'a, I> {

    pub fn next_path(&mut self, ops: &mut Vec<AlignmentOperation>) -> Option<(usize, usize, u8)> {
        self.find_next(Some(ops))
    }

    pub fn find_next(&mut self, ops: Option<&mut Vec<AlignmentOperation>>) -> Option<(usize, usize, u8)> {
        for (i, &a) in self.text.by_ref() {
            self.myers.step_trace(&mut self.state, a);
            if self.state.dist <= self.max_dist {
                let h_offset = self.myers.tb.traceback(ops);
                return Some((i + 1 - h_offset, i + 1, self.state.dist));
            }
        }
        None
    }
}


impl<'a, I: Iterator<Item = &'a u8>> Iterator for FullMatches<'a, I> {
    type Item = (usize, usize, u8);

    fn next(&mut self) -> Option<(usize, usize, u8)> {
        self.find_next(None)
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AlignmentOperation {
    Match,
    Subst,
    Ins,
    Del
}

use self::AlignmentOperation::*;


struct Traceback {
    states: Vec<State>,
    positions: iter::Cycle<Range<usize>>,
    pos: usize,
    m: u8,
}

impl Traceback {
    fn new() -> Traceback {
        Traceback {
            states: vec![],
            positions: (0..0).cycle(),
            m: 0,
            pos: 0,
        }
    }

    fn init(&mut self, m: u8, k: usize) {
        self.m = m;
        let num_cols = m as usize + k + 1;
        self.positions = (0..num_cols).cycle();
        let curr_len = self.states.len();
        if num_cols > curr_len {
            for _ in 0..num_cols - curr_len {
                self.states.push(State::default());
            }
        }
        debug_assert!(self.states.len() >= num_cols);

        let leftmost_state = &mut self.states[0];
        leftmost_state.dist = m;
        leftmost_state.pv = ::std::u64::MAX; // all 1s
        self.pos = self.positions.next().unwrap();
    }

    #[inline]
    fn add_state(&mut self, s: State) {
        self.pos = self.positions.next().unwrap();
        //self.states[self.pos] = s;
        // faster
        *unsafe { self.states.get_unchecked_mut(self.pos) } = s;
    }

    /// Returns the length of the current match, optionally adding the
    /// alignment path to `ops`
    fn traceback(&self, mut ops: Option<&mut Vec<AlignmentOperation>>) -> usize {

        let mut states = self.states
            .iter()
            .rev()
            .cycle()
            .skip(self.states.len() - self.pos - 1);

        let ops = &mut ops;
        if let Some(o) = ops.as_mut() {
            o.clear();
        }

        // bit position that is always tested in move_up!()
        let max_mask = 1 << (self.m - 1);

        macro_rules! move_up {
            ($state:expr) => {
                 if $state.pv & max_mask != 0 {
                     $state.dist -= 1
                 } else if $state.mv & max_mask != 0 {
                     $state.dist += 1
                 }
                // Not always faster:
                //$state.dist += ($state.mv & max_mask != 0) as u8;
                //$state.dist -= ($state.pv & max_mask != 0) as u8;
                $state.pv <<= 1;
                $state.mv <<= 1;
            };
        }

        macro_rules! move_up_many {
            ($state:expr, $n:expr) => {
                let mask = ((1 << $n) - 1) << (self.m - $n);
                $state.dist += (($state.mv & mask)).count_ones() as u8;
                $state.dist -= (($state.pv & mask)).count_ones() as u8;
                $state.mv <<= $n;
                $state.pv <<= $n;

                // equally fast:
                // let n = self.m - $n;
                // let range_mask = (1 << self.m) - 1; // (define once)
                // $state.dist += (($state.mv & range_mask) >> n).count_ones() as u8;
                // $state.dist -= (($state.pv & range_mask) >> n).count_ones() as u8;

                // A loop seems always slower (not sure about systems without popcnt):
                // for _ in 0..$n {
                //     move_up!($state);
                // }
            };
        }

        // horizontal distance from right end
        let mut h_offset = 0;
        // vertical offset from bottom of table
        let mut v_offset = 0;

        // current state
        let mut state = states.next().unwrap().clone();
        // state left to the current state
        let mut lstate = states.next().unwrap().clone();

        while v_offset < self.m {
            let op =
                if state.pv & max_mask != 0 {
                    // up
                    v_offset += 1;
                    move_up!(state);
                    move_up!(lstate);
                    Ins
                } else {
                    let op =
                        if lstate.dist + 1 == state.dist {
                            // left
                            Del
                        } else {
                            // diagonal
                            v_offset += 1;
                            move_up!(lstate);
                            if lstate.dist == state.dist {
                                Match
                            } else {
                                Subst
                            }
                        };
                    // move left
                    state = lstate;
                    lstate = states.next().unwrap().clone();
                    move_up_many!(lstate, v_offset);
                    h_offset += 1;

                    op
                };

            if let Some(o) = ops.as_mut() {
                o.push(op);
            }
        }

        if let Some(o) = ops.as_mut() {
            o.reverse();
        }

        h_offset
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance() {
        let text = b"TGAGCNT";
        let pattern = b"TGAGCGT";

        let myers = Myers::new(pattern);
        assert_eq!(myers.distance(text), 1);

        let myers_wildcard = Myers::with_wildcard(pattern, b'N');
        assert_eq!(myers_wildcard.distance(text), 0);
    }

    #[test]
    fn test_position() {
        let text = b"CAGACATCTT";
        let pattern = b"AGA";

        let mut myers = Myers::new(pattern);
        let matches: Vec<_> = myers.find_all_pos(text, 1).collect();
        assert_eq!(&matches, &[(1, 3, 1), (1, 4, 0), (1, 5, 1), (3, 6, 1)]);
    }

    #[test]
    fn test_traceback() {
        let text =   "CAGA-CAT-CTT".replace('-', "");
        let pattern =   "AGCGTGCT".replace('-', "");

        let mut myers = Myers::new(pattern.as_bytes());
        let mut matches = myers.find_all_pos(text.as_bytes(), 3);
        let mut aln = vec![];
        assert_eq!(matches.find_next(Some(&mut aln)).unwrap(), (3, 9, 3));
        assert_eq!(aln, &[Match, Ins, Match, Subst, Match, Ins, Match, Match]);
    }

    #[test]
    fn test_traceback2() {
        let text =    "TCAG--CAGATGGAGCTC".replace('-', "");
        let pattern = "TCAGAGCAG".replace('-', "");

        let mut myers = Myers::new(pattern.as_bytes());
        let mut matches = myers.find_all_pos(text.as_bytes(), 2);
        let mut aln = vec![];
        assert_eq!(matches.find_next(Some(&mut aln)).unwrap(), (0, 7, 2));
        assert_eq!(aln, &[Match, Match, Match, Match, Ins, Ins, Match, Match, Match]);
    }

    #[test]
    fn test_shorter() {
        let text =     "ATG";
        let pattern = "CATGC";

        let mut myers = Myers::new(pattern.as_bytes());
        let mut matches = myers.find_all_pos(text.as_bytes(), 2);
        let mut aln = vec![];
        assert_eq!(matches.find_next(Some(&mut aln)).unwrap(), (0, 3, 2));
        assert_eq!(aln, &[Ins, Match, Match, Match, Ins]);
    }

    #[test]
    fn test_long_shorter() {
        let text =           "CCACGCGTGGGTCCTGAGGGAGCTCGTCGGTGTGGGGTTCGGGGGGGTTTGT";
        let pattern ="CGCGGTGTCCACGCGTGGGTCCTGAGGGAGCTCGTCGGTGTGGGGTTCGGGGGGGTTTGT";

        let mut myers = Myers::new(pattern.as_bytes());
        let mut matches = myers.find_all_pos(text.as_bytes(), 8);
        assert_eq!(matches.next().unwrap(), (0, 52, 8));
    }

    #[test]
    fn test_ambig() {
        let text =    b"TGABCNT";
        let pattern = b"TGRRCGT";
        //                x  x
        // Matching is asymmetric here (A matches R and G matches N, but the reverse is not true)

        let myers = Myers::from_variants(pattern.into_iter().map(|&b| {
            if b == b'R' { b"AG".to_vec() } else { vec![b] }
        }));
        assert_eq!(myers.distance(text), 2);
    }
}
