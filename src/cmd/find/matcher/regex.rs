use std::fmt;

use crate::error::CliResult;

use super::{Hit, Match, Matcher};

pub fn get_matcher(
    pattern: &str,
    single_hit: bool,
    has_groups: bool,
    case_insensitive: bool,
) -> CliResult<Box<dyn Matcher + Send + Sync>> {
    Ok(Box::new(RegexMatcher::new(
        pattern,
        single_hit,
        has_groups,
        case_insensitive,
    )?))
}

macro_rules! matcher_impl {
    ($re_mod:ident, $text_fn:expr) => {
        #[derive(Debug, Clone)]
        pub struct RegexMatcher {
            capture_locations: Option<$re_mod::CaptureLocations>,
            inner: $re_mod::Regex,
            has_groups: bool,
        }

        impl RegexMatcher {
            pub fn new(
                pattern: &str,
                single_hit: bool,
                has_groups: bool,
                case_insensitive: bool,
            ) -> CliResult<RegexMatcher> {
                let inner = $re_mod::RegexBuilder::new(pattern)
                    .case_insensitive(case_insensitive)
                    .build()?;
                Ok(RegexMatcher {
                    capture_locations: if single_hit {
                        Some(inner.capture_locations())
                    } else {
                        None
                    },
                    inner,
                    has_groups,
                })
            }
        }

        impl Matcher for RegexMatcher {
            fn has_matches(&self, text: &[u8]) -> Result<bool, String> {
                Ok(self.inner.is_match($text_fn(text)?))
            }

            fn do_search(
                &mut self,
                text: &[u8],
                func: &mut dyn FnMut(&dyn Hit) -> Result<bool, String>,
            ) -> Result<(), String> {
                let text = $text_fn(text)?;
                if !self.has_groups {
                    // no allocations
                    for h in self.inner.find_iter(text) {
                        if !func(&h)? {
                            break;
                        }
                    }
                } else if let Some(locs) = self.capture_locations.as_mut() {
                    // only first hit needed
                    if self.inner.captures_read(locs, text).is_some() {
                        func(locs)?;
                    }
                } else {
                    // allocates captures -> slower
                    for h in self.inner.captures_iter(text) {
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

        impl Hit for $re_mod::CaptureLocations {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                let (start, end) = self.get(group).ok_or_else(|| GroupError(group))?;
                out.start = start;
                out.end = end;
                Ok(())
            }
        }

        impl<'t> Hit for $re_mod::Captures<'t> {
            fn get_group(&self, group: usize, out: &mut Match) -> Result<(), String> {
                let g = self.get(group).ok_or_else(|| GroupError(group))?;
                out.start = g.start();
                out.end = g.end();
                Ok(())
            }
        }

        /// Checks for the presence of regex groups and returns their Ok(num) if
        /// the given group number (given as text) is present.
        /// If it is not a number, it is assumed to be a named group, and
        /// the corresponding capture number is returned if the name is found.
        ///
        /// As I understand the documentation of capture_names() and the underlying
        /// code, we should generally be able to correctly resolve names -> group numbers
        /// (capture_names() is in order of group indices).
        ///
        // TODO: this function is always called first and RegexMatcher is created in a later step, so the regex is parsed twice!
        //       Still, this seems acceptable as only groups > 0 require the parsing
        pub fn resolve_group(pattern: &str, group: &str) -> Result<usize, String> {
            if let Ok(num) = group.parse() {
                Ok(num)
            } else {
                let re = $re_mod::Regex::new(pattern)
                    .map_err(|e| format!("Invalid regular expression: {}", e.to_string()))?;
                if let Some(num) = re.capture_names().position(|n| n == Some(group)) {
                    Ok(num)
                } else {
                    Err(format!(
                        "Named regex group '{}' not present in pattern '{}'",
                        group,
                        re.as_str()
                    ))
                }
            }
        }
    };
}

struct GroupError(usize);

impl fmt::Display for GroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regex group no. {} not found", self.0)
    }
}

impl From<GroupError> for String {
    #[cold]
    fn from(e: GroupError) -> String {
        e.to_string()
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "regex-fast")] {
        use regex::bytes as regex_bytes;
        matcher_impl!(regex_bytes, Ok::<_, String>);
    } else {
        matcher_impl!(regex_lite, |t| std::str::from_utf8(t).map_err(|e| e.to_string()));
    }
}
