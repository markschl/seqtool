use crate::error::CliResult;

use super::{Hit, Match, Matcher};

macro_rules! matcher_impl {
    ($name:ident, $re_mod:ident, $text_fn:expr) => {
        pub struct $name {
            re: $re_mod::Regex,
            has_groups: bool,
        }

        impl $name {
            pub fn new(pattern: &str, has_groups: bool) -> CliResult<$name> {
                Ok($name {
                    re: $re_mod::Regex::new(pattern)?,
                    has_groups,
                })
            }
        }

        impl Matcher for $name {
            #[allow(clippy::redundant_closure_call)]
            fn iter_matches(
                &mut self,
                text: &[u8],
                func: &mut dyn FnMut(&dyn Hit) -> bool,
            ) -> CliResult<()> {
                let text = $text_fn(text)?;
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
                Ok(())
            }
        }

        impl<'t> Hit for $re_mod::Match<'t> {
            fn pos(&self) -> (usize, usize) {
                (self.start(), self.end())
            }

            fn group(&self, _: usize) -> Option<Match> {
                Some(Match::new(self.start(), self.end(), 0, 0, 0, 0))
            }
        }

        impl<'t> Hit for $re_mod::Captures<'t> {
            fn pos(&self) -> (usize, usize) {
                self.get(0).unwrap().pos()
            }

            fn group(&self, group: usize) -> Option<Match> {
                self.get(group)
                    .map(|m| Match::new(m.start(), m.end(), 0, 0, 0, 0))
            }
        }
    };
}

cfg_if::cfg_if! {
    if #[cfg(feature = "regex-fast")] {
        use regex::bytes as regex_bytes;
        matcher_impl!(RegexMatcher, regex, |t| std::str::from_utf8(t));
        matcher_impl!(BytesRegexMatcher, regex_bytes, Ok::<_, crate::error::CliError>);
    } else {
        matcher_impl!(RegexMatcher, regex_lite, |t| std::str::from_utf8(t));
        pub type BytesRegexMatcher = RegexMatcher;
    }
}
