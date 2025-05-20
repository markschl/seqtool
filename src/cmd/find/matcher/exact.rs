use memchr::memmem::Finder;

use super::{Hit, Match, Matcher};

#[derive(Debug)]
pub struct ExactMatcher {
    finder: Finder<'static>,
    len: usize,
}

impl ExactMatcher {
    pub fn new(pattern: &[u8]) -> Self {
        Self {
            finder: Finder::new(pattern).into_owned(),
            len: pattern.len(),
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
            if !func(&(start, start + self.len))? {
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
