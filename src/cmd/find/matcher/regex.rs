use crate::error::CliResult;

use super::{Hit, Match, Matcher};

pub fn get_matcher(pattern: &str, has_groups: bool) -> CliResult<Box<dyn Matcher + Send>> {
    Ok(Box::new(RegexMatcher::new(pattern, has_groups)?))
}

macro_rules! matcher_impl {
    ($name:ident, $re_mod:ident, $text_fn:expr) => {
        #[derive(Debug)]
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
            fn has_matches(&self, text: &[u8]) -> Result<bool, String> {
                Ok(self.re.is_match($text_fn(text)?))
            }

            // #[allow(clippy::redundant_closure_call)]
            fn iter_matches(
                &mut self,
                text: &[u8],
                func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
            ) -> Result<(), String> {
                let text = $text_fn(text)?;
                if self.has_groups {
                    // allocates captures
                    for h in self.re.captures_iter(text) {
                        if !func(&h)? {
                            break;
                        }
                    }
                } else {
                    // no allocations
                    for h in self.re.find_iter(text) {
                        if !func(&h)? {
                            break;
                        }
                    }
                }
                Ok(())
            }
        }

        impl<'t> Hit for $re_mod::Match<'t> {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                debug_assert!(group == 0);
                out.start = self.start();
                out.end = self.end();
                Ok(())
            }
        }

        impl<'t> Hit for $re_mod::Captures<'t> {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                let g = self
                    .get(group)
                    .ok_or_else(|| format!("Regex group '{}' not found", group))?;
                out.start = g.start();
                out.end = g.end();
                Ok(())
            }
        }
    };
}

cfg_if::cfg_if! {
    if #[cfg(feature = "regex-fast")] {
        use regex::bytes as regex_bytes;
        matcher_impl!(RegexMatcher, regex_bytes, Ok::<_, String>);
    } else {
        matcher_impl!(RegexMatcher, regex_lite, |t| std::str::from_utf8(t).map_err(|e| e.to_string()));
    }
}
