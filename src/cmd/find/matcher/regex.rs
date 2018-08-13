use error::CliResult;
use regex;

use super::*;

pub struct BytesRegexMatcher {
    re: regex::bytes::Regex,
    has_groups: bool,
}

impl BytesRegexMatcher {
    pub fn new(pattern: &str, has_groups: bool) -> CliResult<BytesRegexMatcher> {
        Ok(BytesRegexMatcher {
            re: regex::bytes::Regex::new(pattern)?,
            has_groups: has_groups,
        })
    }
}

impl Matcher for BytesRegexMatcher {
    fn iter_matches(&mut self, text: &[u8], func: &mut FnMut(&Hit) -> bool) {
        if self.has_groups {
            // allocates captures
            for h in self.re.captures_iter(text) {
                if !func(&h) {
                    break;
                }
            }
        } else {
            // no allocations
            for h in self.re.find_iter(text) {
                if !func(&h) {
                    break;
                }
            }
        }
    }
}

impl<'t> Hit for regex::bytes::Match<'t> {
    fn pos(&self) -> (usize, usize) {
        (self.start(), self.end())
    }

    fn group(&self, _: usize) -> Option<Match> {
        Some(Match::new(self.start(), self.end(), 0, 0, 0, 0))
    }
}

impl<'t> Hit for regex::bytes::Captures<'t> {
    fn pos(&self) -> (usize, usize) {
        self.get(0).unwrap().pos()
    }

    fn group(&self, group: usize) -> Option<Match> {
        self.get(group)
            .map(|m| Match::new(m.start(), m.end(), 0, 0, 0, 0))
    }
}
