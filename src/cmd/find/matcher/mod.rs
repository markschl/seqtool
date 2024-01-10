use std::fmt::Debug;

mod approx;
mod exact;
mod regex;

use crate::error::CliResult;

pub use self::approx::*;
pub use self::exact::*;
pub use self::regex::*;

pub trait Matcher {
    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
    ) -> CliResult<()>;
}

impl<M: Matcher + ?Sized> Matcher for Box<M> {
    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
    ) -> CliResult<()> {
        (**self).iter_matches(text, func)
    }
}

impl<'a, M: Matcher> Matcher for &'a mut M {
    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
    ) -> CliResult<()> {
        (**self).iter_matches(text, func)
    }
}

pub trait Hit: Debug {
    fn pos(&self) -> (usize, usize);
    fn group(&self, group_idx: usize) -> Option<Match>;
}

#[derive(Debug)]
pub struct SimpleHit(Match);

impl Hit for SimpleHit {
    fn pos(&self) -> (usize, usize) {
        (self.0.start, self.0.end)
    }
    fn group(&self, _: usize) -> Option<Match> {
        Some(self.0.clone())
    }
}

/// contains 0-based coordinates and distance
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub dist: u16,
    pub subst: u16,
    pub ins: u16,
    pub del: u16,
}

impl Match {
    pub fn new(start: usize, end: usize, dist: u16, subst: u16, ins: u16, del: u16) -> Match {
        Match {
            start,
            end,
            dist,
            subst,
            ins,
            del,
        }
    }

    pub fn neg_start1(&self, seq_len: usize) -> i64 {
        self.start as i64 - seq_len as i64
    }

    pub fn neg_end1(&self, seq_len: usize) -> i64 {
        self.end as i64 - seq_len as i64 - 1
    }
}
