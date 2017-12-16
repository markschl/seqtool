use itertools::Itertools;

use super::*;

pub struct FuzzyHandler<A>
where
    A: Fn(u8, u8) -> i32,
{
    aligner: Option<Aligner<A>>,
    sort_vec: Option<Vec<Match>>,
    group_pos: bool,
}

impl<A> FuzzyHandler<A>
where
    A: Fn(u8, u8) -> i32 + Copy,
{
    pub fn new(
        pattern: &[u8],
        needs_alignment: bool,
        sorted: bool,
        group_pos: bool,
        score_fn: A,
    ) -> FuzzyHandler<A> {
        FuzzyHandler {
            aligner: if needs_alignment {
                Some(Aligner::new(pattern, score_fn))
            } else {
                None
            },
            sort_vec: if sorted { Some(vec![]) } else { None },
            group_pos: group_pos,
        }
    }

    pub fn get_matches<F, I>(&mut self, iter: I, text: &[u8], mut func: F)
    where
        F: FnMut(Match) -> bool,
        I: Iterator<Item = (usize, u16)>,
    {
        if let Some(ref mut aligner) = self.aligner.as_mut() {
            if self.group_pos {
                // Filter hits which have the same start positions:
                // keep the first one with the smallest distance
                // In case of ties, this means that short hits are preferred over long ones.
                let groups = iter.map(|(end, dist)| {
                    let mut m = Match::new(0, end, dist, 0, 0, 0);
                    aligner.align(text, &mut m);
                    m
                }).group_by(|m| m.start);

                let iter = groups.into_iter().map(|(_, mut it)| {
                    let mut out = None;
                    let mut best_dist = ::std::u16::MAX;
                    while let Some(m) = it.next() {
                        if (m.dist as u16) < best_dist {
                            best_dist = m.dist as u16;
                            out = Some(m.clone());
                        }
                    }
                    out.unwrap()
                });
                //let iter = iter.map(|(end, dist)| aligner.align(text, end, dist));
                opt_sorted(iter, self.sort_vec.as_mut(), |m| m.dist, &mut func);
            } else {
                // no grouping necessary -> do alignment AFTER sorting
                // to minimize the number of necessary alignments
                opt_sorted(
                    iter.map(|(end, dist)| Match::new(0, end, dist, 0, 0, 0)),
                    self.sort_vec.as_mut(),
                    |m| m.dist,
                    |mut m| {
                        aligner.align(text, &mut m);
                        func(m)
                    },
                );
            }
        } else {
            // no alignment necessary
            let iter = iter.map(|(end, dist)| Match::new(0, end, dist, 0, 0, 0));
            opt_sorted(iter, self.sort_vec.as_mut(), |m| m.dist, &mut func);
        }
    }
}


fn opt_sorted<T, U, I, K, F>(iter: I, sort_vec: Option<&mut Vec<T>>, ord_key: K, mut func: F)
where
    I: Iterator<Item = T>,
    K: Fn(&T) -> U,
    U: Ord,
    T: Clone,
    F: FnMut(T) -> bool,
{
    if let Some(v) = sort_vec {
        v.clear();
        v.extend(iter);
        v.sort_by_key(ord_key);
        for item in v {
            if !func(item.clone()) {
                break;
            }
        }
    } else {
        for item in iter {
            if !func(item) {
                break;
            }
        }
    }
}
