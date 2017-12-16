use bio::pattern_matching::ukkonen;

use super::handle::FuzzyHandler;
use super::*;
use error::CliResult;

pub struct UkkonenMatcher<'a, F, A>
where
    F: 'a + Fn(u8, u8) -> u32,
    A: Fn(u8, u8) -> i32,
{
    matcher: ukkonen::Ukkonen<&'a F>,
    max_dist: u16,
    pattern: Vec<u8>,
    handler: FuzzyHandler<A>,
}

impl<'a, C, A> UkkonenMatcher<'a, C, A>
where
    C: Fn(u8, u8) -> u32,
    A: Fn(u8, u8) -> i32 + Copy,
{
    pub fn new(
        pattern: &[u8],
        max_dist: u8,
        needs_alignment: bool,
        sorted: bool,
        group_pos: bool,
        cost_fn: &'a C,
        aln_score_fn: A,
    ) -> CliResult<UkkonenMatcher<'a, C, A>> {
        let h = FuzzyHandler::new(pattern, needs_alignment, sorted, group_pos, aln_score_fn);
        let matcher = ukkonen::Ukkonen::with_capacity(pattern.len(), cost_fn);
        Ok(UkkonenMatcher {
            matcher: matcher,
            max_dist: max_dist as u16,
            pattern: pattern.to_owned(),
            handler: h,
        })
    }
}

impl<'a, C, A> Matcher for UkkonenMatcher<'a, C, A>
where
    C: Fn(u8, u8) -> u32,
    A: Fn(u8, u8) -> i32 + Copy,
{
    fn iter_matches(&mut self, text: &[u8], func: &mut FnMut(&Hit) -> bool) {
        let matches = self.matcher
            .find_all_end(&self.pattern, text, self.max_dist as usize)
            .map(|(end, dist)| (end, dist as u16));

        self.handler.get_matches(matches, text, |m| {
            let h = SimpleHit(m);
            func(&h)
        })
    }
}
