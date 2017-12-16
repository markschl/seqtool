use super::*;
use super::handle::FuzzyHandler;
use error::CliResult;

use bio::pattern_matching::myers;

pub struct MyersMatcher<A>
where
    A: Fn(u8, u8) -> i32,
{
    matcher: myers::Myers,
    max_dist: u8,
    handler: FuzzyHandler<A>,
}

impl<A> MyersMatcher<A>
where
    A: Fn(u8, u8) -> i32 + Copy,
{
    pub fn new(
        pattern: &[u8],
        max_dist: u8,
        needs_alignment: bool,
        sorted: bool,
        group_pos: bool,
        aln_score_fn: A,
    ) -> CliResult<MyersMatcher<A>> {
        let matcher = myers::Myers::new(pattern);

        let h = FuzzyHandler::new(pattern, needs_alignment, sorted, group_pos, aln_score_fn);
        Ok(MyersMatcher {
            matcher: matcher,
            max_dist: max_dist,
            handler: h,
        })
    }
}

impl<A> Matcher for MyersMatcher<A>
where
    A: Fn(u8, u8) -> i32 + Copy,
{
    fn iter_matches(&mut self, text: &[u8], func: &mut FnMut(&Hit) -> bool) {
        let matches = self.matcher
            .find_all_end(text, self.max_dist)
            .map(|(end, dist)| (end, dist as u16));

        self.handler.get_matches(matches, text, |m| {
            let h = SimpleHit(m);
            func(&h)
        })
    }
}
