use bio::{alignment::AlignmentOperation, pattern_matching::myers};

use crate::cmd::find::ambig::{AMBIG_DNA, AMBIG_PROTEIN, AMBIG_RNA};
use crate::cmd::find::opts::{DistanceThreshold, Opts, RequiredInfo};
use crate::CliResult;

use super::{Hit, Match, Matcher};

// Scores to use to select the best alignment amonst multiple hits with the
// same edit distance
const GAP_PENALTY: isize = 3;

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
}

impl MyersOpts {
    pub fn new(
        max_dist: usize,
        sort_by_dist: bool,
        max_hits: usize,
        required_info: RequiredInfo,
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
        }
    }
}

pub fn get_matcher(pattern: &str, ambig: bool, opts: &Opts) -> CliResult<Box<dyn Matcher + Send>> {
    let ambig_map = if ambig {
        use crate::helpers::seqtype::SeqType::*;
        match opts.seqtype {
            Dna => Some(AMBIG_DNA),
            Rna => Some(AMBIG_RNA),
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
            opts: MyersOpts::new(max_dist, !opts.in_order, opts.max_hits, opts.required_info),
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

    // while the Myers algorithm always reports all possible distinct end positions,
    // several hits usually share the same start position due to
    // insertions/deletions. The grouping makes sure that for each start position,
    // only the optimal hit is reported, regardless on whether the hits are
    // ordered by position or by distance.
    fn iter_matches(
        &mut self,
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

            // other cases: either range or alignment of hits needed, or multiple hits
            // in any case, we need to calculate the traceback for part of them (thus, we need `find_all_lazy`)
            let mut matches = $myers.find_all_lazy(text, opts.max_dist as $dist_ty);

            // calculates alignment score using a custom scoring that penalizes InDels more than
            // with the edit distance
            macro_rules! calc_score {
                ($pos:expr) => {
                    {
                        path_buf.clear();  // TODO: should it be cleared in bio API already?
                        let (start, dist) = matches.path_at($pos, path_buf).unwrap();
                        let score: isize = path_buf.iter().map(|op| {
                            use AlignmentOperation::*;
                            match op {
                                Match => 0,
                                Ins | Del => -GAP_PENALTY,
                                Subst => -1,
                                _ => unreachable!(),
                            }
                        }).sum();
                        // dbg!((start, i, best_dist, score));
                        (start, dist, score)
                    }
                }
            }

            macro_rules! report_hit {
                ($hit:expr) => {
                    if opts.needs_alignment {
                        func(&mut ($hit.1, &matches))
                    } else {
                        func(&mut ($hit.0, $hit.1 + 1, $hit.2))
                    }
                }
            }

            if opts.best_only {
                // Only the hit with the smallest distance requested:
                // this allows for a faster implementation:
                // (1) Obtain the hit with the smallest edit distance.
                //   There may be more possible alignments with end positions
                //   more shifted to the right, but same starting position
                //   *and* the same edit distance. From those, we select the
                //   hit using a custom scoring function with a higher gap penalty.
                if let Some((end, best_dist)) = matches.by_ref().min_by_key(|&(_, dist)| dist) {
                    // (2) Search for additional hits with the same starting
                    //   position within the range of possible positions.
                    //   The range of possible end positions is: end .. end + 2 * best_dist + 1
                    //    whereby 'end' is the end of the *leftmost* 'best' hit
                    let mut best_pos = (usize::MAX, usize::MAX);
                    let mut best_score = isize::MIN;
                    for i in end..std::cmp::min(end + 2 * best_dist as usize + 1, text.len()) {
                        // TODO: we need to calculate a traceback to obtain the distance,
                        //       but it is actually already known (internally) -> issue a PR to rust-bio
                        //       for a `LazyMatches::dist_at()` method
                        let (start, dist, score) = calc_score!(i);
                        // eprintln!("f {}..{} -> d {} (s {})", start, i, dist, score);
                        // matches.alignment_at(i, &mut aln);
                        // println!("{}", aln.pretty(pattern, text, 80));
                        if start == best_pos.0 || best_pos.0 == usize::MAX {
                            if dist == best_dist && score > best_score {
                                // found a better hit (higher score) with the same edit distance
                                best_pos = (start, i);
                                best_score = score;
                            }
                        } else {
                            // not the same starting position -> done
                            break;
                        }
                    }
                    debug_assert!(best_pos.0 != usize::MAX);
                    // TODO: +1?
                    // black_box(best_pos);
                    // black_box(matches.alignment_at(best_pos.1, &mut aln));
                    // println!("final\n{}", aln.pretty(pattern, text, 80));
                    let do_continue = report_hit!((best_pos.0, best_pos.1, best_dist as usize))?;
                    debug_assert!(!do_continue);
                }
            } else {
                // Multiple hits requested, either in-order or sorted by distance.
                // In both cases, we first collect all possible hits.
                // In case of hits with different end positions, but the same
                // starting positions and identical minimum edit distance,
                // we again select the best one using a scoring function.
                // This approach requires obtaining the alignment path for
                // *every* reported hit, and while proceeding we obtain
                // the hit with the best score for every starting position.
                if opts.sort_by_dist {
                    sort_vec.clear();
                }

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

                let mut best_score = isize::MIN;
                let mut best_hit = (usize::MAX, usize::MAX, usize::MAX);
                while let Some((end, dist)) = matches.next() {
                    let (start, _dist, score) = calc_score!(end);
                    debug_assert!(dist == _dist);
                    // eprintln!("f ({}, {}) -> d {} (s {})", start, end, dist, score);
                    // let mut aln = bio::alignment::Alignment::default();
                    // matches.alignment_at(end, &mut aln);
                    // println!("{}", aln.pretty(b"A", text, 80));
                    if start != best_hit.0 {
                        // new hit with different starting position
                        if best_score != isize::MIN && !report_push_hit!(best_hit)? {
                            break;
                        }
                        best_score = score;
                        best_hit = (start, end, dist as usize);
                    } else if score > best_score {
                        // new best score found for current starting position
                        best_score = score;
                        best_hit = (start, end, dist as usize);
                    }
                }
                // add last hit (if any)
                if best_score != isize::MIN {
                    report_push_hit!(best_hit)?;
                }

                if opts.sort_by_dist {
                    // report hits sorted by distance
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

impl Hit for (usize, usize, usize) {
    fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
        debug_assert!(group == 0);
        out.start = self.0;
        out.end = self.1;
        out.dist = self.2;
        Ok(())
    }
}

macro_rules! impl_aln_hit {
    ($($part:ident)::*, $wrapper_name:ident) => {
        impl<'a, C, I> Hit for (usize, & $($part)::*::LazyMatches<'a, u64, C, I>)
        where
           C: std::borrow::Borrow<u8>,
           I: Iterator<Item = C> + ExactSizeIterator,
        {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                debug_assert!(group == 0);
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

impl_aln_hit!(myers, LazyMatchesWrapper);
impl_aln_hit!(myers::long, LongLazyMatchesWrapper);
