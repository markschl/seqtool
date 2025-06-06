use memchr::memmem::Finder;

use super::{Hit, Match, Matcher};

#[derive(Debug, Clone)]
pub struct ExactMatcher {
    finder: Finder<'static>,
    pattern_len: usize,
}

impl ExactMatcher {
    pub fn new(pattern: &[u8]) -> Self {
        Self {
            finder: Finder::new(pattern).into_owned(),
            pattern_len: pattern.len(),
        }
    }
}

impl Matcher for ExactMatcher {
    fn has_matches(&self, text: &[u8]) -> Result<bool, String> {
        Ok(self.finder.find_iter(text).next().is_some())
    }

    fn do_search(
        &mut self,
        text: &[u8],
        func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
    ) -> Result<(), String> {
        for start in self.finder.find_iter(text) {
            if !func(&(start, start + self.pattern_len))? {
                break;
            }
        }
        Ok(())
    }
}

impl Hit for (usize, usize) {
    fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
        debug_assert!(group == 0);
        out.start = self.0;
        out.end = self.1;
        Ok(())
    }
}
