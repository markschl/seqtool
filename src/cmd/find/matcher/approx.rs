
use super::*;
use error::CliResult;
use std::collections::HashMap;

use itertools::Itertools;

use bio::pattern_matching::myers::{Myers, MyersBuilder, BitVec};

pub struct MyersMatcher<T: BitVec> {
    myers: Myers<T>,
    max_dist: u8,
    needs_start: bool,
    sort_vec: Option<Vec<Match>>,
}

impl<T: BitVec> MyersMatcher<T> {
    pub fn new(
        pattern: &[u8],
        max_dist: u8,
        needs_start: bool,
        sorted: bool,
        ambig_trans: Option<&HashMap<u8, Vec<u8>>>,
    ) -> CliResult<Self>
    {
        let mut builder = MyersBuilder::new();
        if let Some(trans) = ambig_trans {
            for (&symbol, equivalents) in trans {
                builder.ambig(symbol, equivalents);
            }
        }

        Ok(MyersMatcher {
            myers: builder.build(pattern),
            max_dist: max_dist,
            needs_start: needs_start,
            sort_vec: if sorted { Some(vec![]) } else { None },
        })
    }
}

impl<T> Matcher for MyersMatcher<T>
where T: BitVec<DistType=u8>,

{
    fn iter_matches(&mut self, text: &[u8], func: &mut FnMut(&Hit) -> bool) {
        if self.needs_start {
            // group hits by start position
            let by_start = self
                .myers
                .find_all(text, self.max_dist)
                .group_by(|&(start, _, _)| start);

            let iter = by_start
                .into_iter()
                .map(|(_, it)| {
                    let mut out = None;
                    let mut best_dist = ::std::u8::MAX;
                    for m in it {
                        if (m.2) < best_dist {
                            best_dist = m.2;
                            out = Some(m);
                        }
                    }
                    out.unwrap()
                })
                .map(|(start, end, dist)| Match::new(start, end, u16::from(dist), 0, 0, 0));

            opt_sorted(
                iter,
                self.sort_vec.as_mut(),
                |m| m.dist,
                |m| {
                    let h = SimpleHit(m);
                    func(&h)
                },
            );
        } else {
            // only end position needed
            let iter = self
                .myers
                .find_all_end(text, self.max_dist)
                .map(|(end, dist)| Match::new(0, end + 1, u16::from(dist), 0, 0, 0));

            opt_sorted(
                iter,
                self.sort_vec.as_mut(),
                |m| m.dist,
                |m| {
                    let h = SimpleHit(m);
                    func(&h)
                },
            );
        }
    }
}

#[inline(always)]
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
