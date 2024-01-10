use crate::{cmd::shared::twoway_iter::TwowayIter, error::CliResult};

use super::{Hit, Match, Matcher, SimpleHit};

pub struct ExactMatcher(Vec<u8>);

impl ExactMatcher {
    pub fn new(pattern: &[u8]) -> ExactMatcher {
        ExactMatcher(pattern.to_owned())
    }
}

impl Matcher for ExactMatcher {
    fn iter_matches(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> bool,
    ) -> CliResult<()> {
        let l = self.0.len();
        for start in TwowayIter::new(text, &self.0) {
            let h = SimpleHit(Match::new(start, start + l, 0, 0, 0, 0));
            if !func(&h) {
                break;
            }
        }
        Ok(())
    }
}
