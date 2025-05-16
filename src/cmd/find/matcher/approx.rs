use bio::{alignment::AlignmentOperation, pattern_matching::myers};

use crate::cmd::find::{
    ambig::{AMBIG_DNA, AMBIG_PROTEIN, AMBIG_RNA},
    opts::{DistanceThreshold, Opts, RequiredInfo},
};
use crate::CliResult;

use super::{Hit, Match, Matcher};

#[derive(Debug, Clone)]
struct MyersOpts {
    /// maximum edit distance
    max_dist: usize,
    /// sort by distance
    sort_by_dist: bool,
    /// only the best hit needed
    best_only: bool,
    /// only distance of best hit needed?
    best_dist_only: bool,
    /// alignment path needed?
    needs_alignment: bool,
    /// gap penalty for selecting among multiple best hits with the same edit distance
    gap_penalty: u32,
}

impl MyersOpts {
    pub fn new(
        max_dist: usize,
        sort_by_dist: bool,
        max_hits: usize,
        required_info: RequiredInfo,
        gap_penalty: u32,
    ) -> Self {
        if required_info == RequiredInfo::Exists {
            debug_assert!(max_hits == 0);
        }
        Self {
            max_dist,
            sort_by_dist,
            best_only: max_hits == 1,
            best_dist_only: max_hits == 1 && required_info == RequiredInfo::Distance,
            needs_alignment: required_info == RequiredInfo::Alignment,
            gap_penalty,
        }
    }
}

pub fn get_matcher(pattern: &str, ambig: bool, opts: &Opts) -> CliResult<Box<dyn Matcher + Send>> {
    let ambig_map = if ambig {
        use crate::helpers::seqtype::SeqType::*;
        match opts.seqtype {
            DNA => Some(AMBIG_DNA),
            RNA => Some(AMBIG_RNA),
            Protein => Some(AMBIG_PROTEIN),
            Other => None,
        }
    } else {
        None
    };
    let max_dist = match opts.max_dist {
        Some(DistanceThreshold::Diffs(d)) => d,
        Some(DistanceThreshold::DiffRate(r)) => (r * pattern.len() as f64) as usize,
        None => 0,
    };
    Ok(Box::new(MyersMatcher::new(
        pattern.as_bytes(),
        max_dist,
        opts,
        ambig_map,
    )?))
}

#[derive(Debug)]
pub struct MyersMatcher {
    myers: MyersMatcherInner,
    // pattern: Vec<u8>,  // only for debugging
    opts: MyersOpts,
    dist_sort_vec: Vec<(usize, usize, usize)>,
    path_buf: Vec<AlignmentOperation>,
}

impl MyersMatcher {
    pub fn new(
        pattern: &[u8],
        max_dist: usize,
        opts: &Opts,
        ambig_trans: Option<&[(u8, &[u8])]>,
    ) -> CliResult<Self> {
        Ok(MyersMatcher {
            myers: MyersMatcherInner::new(pattern, ambig_trans),
            // pattern: pattern.to_vec(),
            opts: MyersOpts::new(
                max_dist,
                !opts.in_order,
                opts.max_hits,
                opts.required_info,
                opts.gap_penalty,
            ),
            dist_sort_vec: Vec::new(),
            path_buf: Vec::new(),
        })
    }
}

impl Matcher for MyersMatcher {
    fn has_matches(&self, text: &[u8]) -> Result<bool, String> {
        Ok(self.myers.has_matches(text, &self.opts))
    }

    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
    ) -> Result<(), String> {
        self.myers.iter_matches(
            // &self.pattern,
            text,
            func,
            &self.opts,
            &mut self.dist_sort_vec,
            &mut self.path_buf,
        )
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum MyersMatcherInner {
    Simple(myers::Myers<u64>),
    Long(myers::long::Myers<u64>),
}

impl MyersMatcherInner {
    fn new(pattern: &[u8], ambig_trans: Option<&[(u8, &[u8])]>) -> Self {
        let mut builder = myers::MyersBuilder::new();
        if let Some(trans) = ambig_trans {
            for (symbol, equivalents) in trans {
                builder.ambig(*symbol, *equivalents);
            }
        }
        if pattern.len() <= 64 {
            Self::Simple(builder.build(pattern))
        } else {
            Self::Long(builder.build_long(pattern))
        }
    }

    fn has_matches(&self, text: &[u8], opts: &MyersOpts) -> bool {
        macro_rules! impl_has_matches {
            ($myers:expr, $dist_ty:ty) => {
                $myers
                    .find_all_end(text, opts.max_dist as $dist_ty)
                    .next()
                    .is_some()
            };
        }
        match self {
            Self::Simple(m) => impl_has_matches!(m, u8),
            Self::Long(m) => impl_has_matches!(m, usize),
        }
    }

    /// Iterate over all matches (or if opts.best_only, just the best match)
    /// and call `func` on each match.
    ///
    /// `sort_vec`: if not None, collect matches into Some(vec) and sort them by distance.
    /// If None, iterate in order of occurrence.
    ///
    /// The Myers algorithm reports all possible distinct *end* positions of hits
    /// within `opts.max_dist` in-order. The start position is obtained through
    /// backtracking. Thereby, several hits may have the same start position.
    /// Among those, we select the best hit by minimizing a penalty function,
    /// which allows penalizing InDels compared to substitutions.
    /// Only one hit is reported per start position, and multiple hits are
    /// by consequence usually non-overlapping.
    fn iter_matches(
        &mut self,
        // pattern: &[u8],
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
        opts: &MyersOpts,
        sort_vec: &mut Vec<(usize, usize, usize)>,
        path_buf: &mut Vec<AlignmentOperation>,
    ) -> Result<(), String> {
        macro_rules! impl_iter_matches { ($myers:expr, $dist_ty:ty) => { {
            // dbg!(&opts);
            // simplest case: only minimum distance is needed (`find_all_end` is very fast)
            if opts.best_dist_only {
                if let Some((_, dist)) = $myers.find_all_end(text, opts.max_dist as $dist_ty).min_by_key(|&(_, dist)| dist) {
                    func(&mut (0, 0, dist as usize))?;
                }
                return Ok(());
            }

            // eprintln!("find {} in {} (max dist = {})", std::str::from_utf8(pattern).unwrap(), std::str::from_utf8(text).unwrap(), opts.max_dist);
            // let mut aln = bio::alignment::Alignment::default();

            // Other cases: either range or alignment of hits needed, or multiple hits
            // in any case, we need to calculate the aligment for the best hit or all hits
            // (thus, we use `Myers::find_all_lazy`)
            let mut matches = $myers.find_all_lazy(text, opts.max_dist as $dist_ty);

            // Calculates an alignment path and a custom penalty score
            // used to select the final alignment in case of multiple hits with the same edit distance
            macro_rules! get_aligment {
                ($pos:expr) => {
                    {
                        path_buf.clear();  // TODO: should it be cleared in bio API already?
                        let (start, dist) = matches.path_at_reverse($pos, path_buf).unwrap();
                        let penalty: usize = path_buf.iter().map(|op| {
                            use AlignmentOperation::*;
                            match op {
                                Match => 0,
                                Ins | Del => opts.gap_penalty as usize,
                                Subst => 1,
                                _ => unreachable!(),
                            }
                        }).sum();
                        (start, dist, penalty)
                    }
                }
            }

            macro_rules! report_hit {
                ($hit:expr) => {
                    // see Hit implementations below
                    if opts.needs_alignment {
                        // (end, &LazyMatches)
                        // where start <= i <= end  (start obtained from LazyMatches::path_at(end))
                        func(&mut ($hit.1, &matches))
                    } else {
                        // (start, end, edit dist.)
                        // where start <= i <= end
                        func(&mut ($hit.0, $hit.1, $hit.2))
                    }
                }
            }

            if opts.best_only {
                // Only the hit with the smallest distance requested:
                //
                // (1) Obtain the hit with the smallest edit distance
                //   (`min_by_key` returns the hit with the *leftmost* end position
                //   in case of multiple equally good hits).
                if let Some((end0, best_dist)) = matches.by_ref().min_by_key(|&(_, dist)| dist) {
                    // (2) Starting from this leftmost hit, we look further ahead for additional
                    //   hits with the *same start position* and edit distance (`best_dist`).
                    //   We need to consider the full range of theoretically possible end positions
                    //   (end0 <= i <= end1).
                    //
                    //   end1 = end0 + 2 * best_dist
                    //
                    //   *Explanation* with example pattern (length = 6) and best_dist = 2:
                    //
                    //   The shortest hit with the leftmost possible end position can have
                    //   a maximum of `best_dist` insertions in the matched text
                    //   (the matched text cannot be shorter than 6 - 2 = 4 characters):
                    //
                    //    PPPPPP
                    //   TTT-T-TTTTTT    (end0 = 1, end1 = 4)
                    //   012-3-456789
                    //
                    //   The longest possible hit with the rightmost end position
                    //   can have a maximum of `best_dist` insertions in the pattern
                    //   (= `best_dist` deletions in the matched text) and the
                    //   matched text can be max. 6 + 2 = 8 characters long:
                    //
                    //    PPPPP--P
                    //   TTTTTTTTTT    (range 1-8, end1 = 4+2*2 = 8)
                    //   0123456789

                    // This will be: (start, end, edit dist.) where start <= i <= end
                    let mut hit = (usize::MAX, end0, best_dist as usize);
                    let mut lowest_penalty = usize::MAX;
                    // end0 <= i < end0 + 2 * best_dist + 1   [upper bound not included, thus +1]
                    assert!(end0 < text.len());
                    for i in end0..std::cmp::min(end0 + 2 * best_dist as usize + 1, text.len()) {
                        // TODO: we need to calculate the alignment path to also obtain the edit distance
                        //       even if the distance is > best_dist. But but it is actually already known
                        //        (internally) -> issue a PR to rust-bio for a `LazyMatches::dist_at()` method
                        let (start, dist, penalty) = get_aligment!(i);

                        // eprintln!("Hit at {}-{}; edit distance = {} (penalty score = {})", start+1, i+1, dist, penalty);
                        // matches.alignment_at(i, &mut aln);
                        // eprintln!("{}", aln.pretty(pattern, text, 120));

                        if hit.0 == usize::MAX {
                            // first iteration = leftmost hit:
                            // set start position and penalty
                            assert_eq!(dist, best_dist);
                            assert_eq!(i, end0);
                            hit.0 = start;
                            lowest_penalty = penalty;
                        } else if start == hit.0 {
                            // still the same start position
                            if dist == best_dist && penalty < lowest_penalty {
                                // found a better hit with the same edit distance,
                                // but a lower penalty score
                                hit.1 = i;
                                lowest_penalty = penalty;
                            }
                        } else {
                            // not the same start position -> done
                            // (this would be a new hit to report, but anyway we only report one hit)
                            break;
                        }
                    }
                    assert!(hit.0 != usize::MAX);  // should be true if end0 < text.len()

                    // // the following code checks *all* hits with the same start and edit distance
                    // // (should give the same result as above)
                    // let (lstart, ldist, lpenalty) = get_aligment!(end0);
                    // let _all_hits: Vec<_> = (0..text.len()).into_iter().map(|e| (get_aligment!(e), e)).filter(|&((s, _, d), _)| s == lstart && d == ldist as usize).collect();
                    // let ((bstart, bdist, bpenalty), bend) = _all_hits.iter().min_by_key(|&((_, _, penalty), _)| penalty).unwrap();
                    // assert_eq!(*bdist, best_dist);
                    // let hit = (*bstart, *bend, best_dist);
                    // // assert_eq!(*bstart, lstart);
                    // matches.alignment_at(end0, &mut aln);
                    // eprintln!("leftmost rng={}-{} dist={} penalty={}\n{}", lstart, end0, ldist, lpenalty, aln.pretty(pattern, text, 120));
                    // matches.alignment_at(*bend, &mut aln);
                    // eprintln!("chosen   rng={}-{} dist={} penalty={}\n{}", bstart, bend, bdist, bpenalty, aln.pretty(pattern, text, 120));

                    // eprintln!("final\n{}", aln.pretty(pattern, text, 120));

                    let do_continue = report_hit!(hit)?;
                    assert!(!do_continue);
                }
            } else {
                // Multiple hits requested, either in-order or sorted by distance.
                // In both cases, we first collect *all* possible hits,
                // or at least up to the requested number (if in-order, not sorted by distance).
                // Hits are grouped by start position, and only the one hit with the
                // lowest edit distance and the lowest penalty score is reported.

                if opts.sort_by_dist {
                    sort_vec.clear();
                }

                // macro for either directly reporting a hit or adding it to `sort_vec`
                macro_rules! report_push_hit {
                    ($hit:expr) => {
                        if opts.sort_by_dist {
                            // eprintln!("report push {:?}", $hit);
                            sort_vec.push($hit);
                            Ok(true)
                        } else {
                            report_hit!($hit)
                        }
                    }
                }

                let mut lowest_penalty = usize::MAX;
                // (start, end, edit dist.)  where start <= i <= end
                let mut hit = (usize::MAX, usize::MAX, usize::MAX);

                while let Some((end, dist)) = matches.next() {
                    let (start, _dist, penalty) = get_aligment!(end);
                    assert!(dist == _dist);

                    // eprintln!("Hit at {}-{}; edit distance = {} (penalty score = {})", start+1, end+1, dist, penalty);
                    // matches.alignment_at(end, &mut aln);
                    // eprintln!("{}", aln.pretty(pattern, text, 80));

                    if start != hit.0 {
                        // start position is different
                        // -> report the previous hit (if any) and
                        //    stop if no more hits are needed (in case of in-order reporting)
                        if lowest_penalty != usize::MAX && !report_push_hit!(hit)? {
                            break;
                        }
                        // initialize the new hit
                        lowest_penalty = penalty;
                        hit = (start, end, dist as usize);
                    } else if (dist as usize) < hit.2 || (dist as usize) == hit.2 && penalty < lowest_penalty {
                        // new best hit with same start position found:
                        // either a hit with a smaller edit distance
                        // or with the same edit distance and a lower penalty score
                        lowest_penalty = penalty;
                        assert_eq!(hit.0, start);
                        hit.1 = end;
                        hit.2 = dist as usize;
                    }
                }

                // add last hit (if any)
                if lowest_penalty != usize::MAX {
                    report_push_hit!(hit)?;
                }

                // report hits sorted by distance
                if opts.sort_by_dist {
                    sort_vec.sort_by_key(|&(_, _, d)| d);
                    for hit in sort_vec {
                        if !report_hit!(hit)? {
                            break;
                        }
                    }
                }
            }
            Ok(())
        } } }

        match self {
            Self::Simple(m) => impl_iter_matches!(m, u8),
            Self::Long(m) => impl_iter_matches!(m, usize),
        }
    }
}

// `Hit` implementation for (start, end, distance)
// where start <= i <= end
impl Hit for (usize, usize, usize) {
    fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
        debug_assert!(group == 0); // only full hit (group = 0)
        out.start = self.0;
        out.end = self.1 + 1;
        out.dist = self.2;
        Ok(())
    }
}

// `Hit` implementation for (end, &`LazyMatches`)
// where start <= i <= end
// (start is obtained from LazyMatches::path_at(end))
macro_rules! impl_aln_hit {
    ($($path_segment:ident)::*) => {
        impl<'a, C, I> Hit for (usize, & $($path_segment)::*::LazyMatches<'a, u64, C, I>)
        where
           C: std::borrow::Borrow<u8>,
           I: Iterator<Item = C> + ExactSizeIterator,
        {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                debug_assert!(group == 0);  // only full hit (group = 0)
                out.alignment_path.clear();
                let (start, dist) = self.1.path_at(self.0, &mut out.alignment_path).unwrap();
                out.start = start;
                out.end = self.0 + 1;
                out.dist = dist as usize;
                Ok(())
            }
        }
    }
}

impl_aln_hit!(myers);
impl_aln_hit!(myers::long);
