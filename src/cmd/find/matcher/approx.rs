use bio::pattern_matching::myers;
use itertools::Itertools;

use crate::error::CliResult;

use super::{Hit, Match, Matcher, SimpleHit};

#[allow(clippy::large_enum_variant)]
enum _Myers {
    Simple(myers::Myers<u64>),
    Long(myers::long::Myers<u64>),
}

impl _Myers {
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

    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
        needs_start: bool,
        max_dist: usize,
        sort_vec: Option<&mut Vec<Match>>,
    ) {
        macro_rules! _iter_matches {
            ($myers:expr, $dist_ty:ty) => {
                if needs_start {
                    // group hits by start position
                    let by_start = $myers
                        .find_all(text, max_dist as $dist_ty)
                        .group_by(|&(start, _, _)| start);

                    let iter = by_start
                        .into_iter()
                        .map(|(_, it)| {
                            let mut out = None;
                            let mut best_dist = <$dist_ty>::MAX;
                            for m in it {
                                if (m.2) < best_dist {
                                    best_dist = m.2;
                                    out = Some(m);
                                }
                            }
                            out.unwrap()
                        })
                        .map(|(start, end, dist)| Match::new(start, end, dist as u16, 0, 0, 0));

                    opt_sorted(
                        iter,
                        sort_vec,
                        |m| m.dist,
                        |m| {
                            let h = SimpleHit(m);
                            func(&h)
                        },
                    );
                } else {
                    // only end position needed
                    let iter = $myers
                        .find_all_end(text, max_dist as $dist_ty)
                        .map(|(end, dist)| Match::new(0, end + 1, dist as u16, 0, 0, 0));

                    opt_sorted(
                        iter,
                        sort_vec,
                        |m| m.dist,
                        |m| {
                            let h = SimpleHit(m);
                            func(&h)
                        },
                    );
                }
            };
        }

        match self {
            Self::Simple(m) => _iter_matches!(m, u8),
            Self::Long(m) => _iter_matches!(m, usize),
        }
    }
}

pub struct MyersMatcher {
    myers: _Myers,
    max_dist: usize,
    needs_start: bool,
    sort_vec: Option<Vec<Match>>,
}

impl MyersMatcher {
    pub fn new(
        pattern: &[u8],
        max_dist: usize,
        needs_start: bool,
        sorted: bool,
        ambig_trans: Option<&[(u8, &[u8])]>,
    ) -> CliResult<Self> {
        Ok(MyersMatcher {
            myers: _Myers::new(pattern, ambig_trans),
            max_dist,
            needs_start,
            sort_vec: if sorted { Some(vec![]) } else { None },
        })
    }
}

impl Matcher for MyersMatcher {
    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
    ) -> CliResult<()> {
        self.myers.iter_matches(
            text,
            func,
            self.needs_start,
            self.max_dist,
            self.sort_vec.as_mut(),
        );
        Ok(())
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
