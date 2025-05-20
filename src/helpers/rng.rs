use std::str::FromStr;

use memchr::memmem;

use super::{number::parse_int, NA};

/// General and simple range type used in this crate
/// Unbounded ranges that can be negative (viewed from end of sequence).
/// They should behave exactly like Python indexing (slicing) indices, which
/// can be negative as well.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Range {
    pub start: Option<isize>,
    pub end: Option<isize>,
}

impl Range {
    #[cfg(test)]
    pub fn new1(start: Option<isize>, end: Option<isize>) -> Result<Self, String> {
        Self::new(start, end).adjust(false, false)
    }

    #[cfg(test)]
    pub fn new0(start: Option<isize>, end: Option<isize>) -> Self {
        Self::new(start, end)
    }

    pub fn new(start: Option<isize>, end: Option<isize>) -> Self {
        Self { start, end }
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, String> {
        let delim = b":";
        if let Some(delim_pos) = memmem::find(b, delim) {
            let start = trim_ascii(&b[..delim_pos]);
            let end = trim_ascii(&b[delim_pos + delim.len()..]);
            if memmem::find(end, delim).is_none() {
                let start = if start.is_empty() || start == NA.as_bytes() {
                    None
                } else {
                    Some(parse_int(start).map_err(|_| {
                        format!("Invalid range start: '{}'", String::from_utf8_lossy(start))
                    })? as isize)
                };
                let end = if end.is_empty() || end == NA.as_bytes() {
                    None
                } else {
                    Some(parse_int(end).map_err(|_| {
                        format!("Invalid range end: '{}'", String::from_utf8_lossy(end))
                    })? as isize)
                };
                return Ok(Self { start, end });
            }
        }
        Err(format!(
            "Invalid range: '{}'. Possible notations: 'start:end', 'start:', ':end', or ':'",
            String::from_utf8_lossy(b)
        ))
    }

    /// Normalizes a range to 0-based coordinates (used in seqtool internally),
    /// shifting the start or negative end coordinate if the range is 1-based (!range0),
    /// and adjusting the coordinates in case of exclusive range
    pub fn adjust(mut self, range0: bool, exclusive: bool) -> Result<Self, String> {
        if !range0 {
            // start0 = start - 1
            // '3:6' becomes '2:6'
            // '0:6' stays '0:6'  (0 is not a valid 1-based coordinate but may rather be interpreted as open range?)
            if let Some(s) = self.start.as_mut() {
                if *s >= 1 {
                    *s -= 1;
                }
            }
            // negative end coordinates: end0 = end + 1
            // ':-3' becomes ':-2'
            // ':-1' becomes None (end not trimmed)
            if self.end == Some(-1) {
                self.end = None;
            } else if let Some(e) = self.end.as_mut() {
                if *e <= -2 {
                    *e += 1;
                }
            }
        }
        if exclusive {
            // exclusive range: we leave open ranges as-is and only adjust actual coordinates
            if let Some(s) = self.start.as_mut() {
                *s += 1;
            }
            if let Some(e) = self.end.as_mut() {
                if *e != 0 {
                    *e -= 1;
                }
            }
        }
        Ok(self)
    }

    /// Resolves a range with respect to a given sequence length,
    /// converting negative coordinates to standard 0-based coordinates.
    /// Coordinates outside of the sequence range are silently adjusted
    pub fn resolve(&self, length: usize) -> (usize, usize) {
        // resolve negative bounds
        let mut start = self.start.unwrap_or(0);
        if start < 0 {
            start = (length as isize + start).max(0);
        }
        let mut end = self.end.unwrap_or(length as isize);
        if end < 0 {
            end = (length as isize + end).max(0);
        }
        if start > length as isize {
            // silently set start and end to length if both are outside of range,
            // resulting in empty slice
            start = length as isize;
            end = length as isize;
        } else if end > length as isize {
            // silently set end to length if outside of range to avoid panics
            end = length as isize;
        }
        // silently adjust the end bound if it is smaller than the start
        // we don't want a hard error, just return an empty slice
        // TODO: make configurable?
        if end < start {
            // this will result in an empty slice
            end = start;
        }
        (start as usize, end as usize)
    }
}

// Code copied from standard library. Will be removed when slice::trim_ascii
// is stabilized (https://github.com/rust-lang/rust/issues/94035)
#[inline]
pub const fn trim_ascii(bytes: &[u8]) -> &[u8] {
    trim_ascii_end(trim_ascii_start(bytes))
}

pub const fn trim_ascii_start(mut bytes: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = bytes {
        if first.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

#[inline]
const fn trim_ascii_end(mut bytes: &[u8]) -> &[u8] {
    while let [rest @ .., last] = bytes {
        if last.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

impl FromStr for Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng() {
        // 0-based range as input
        assert_eq!(Range::new0(Some(3), Some(10)).resolve(10), (3, 10));
        // start > end -> range adjusted
        assert_eq!(Range::new0(Some(5), Some(4)).resolve(10), (5, 5));
        // 1-based range as input
        assert_eq!(Range::new1(Some(4), Some(10)).unwrap().resolve(10), (3, 10));
        assert_eq!(Range::new1(Some(4), Some(-1)).unwrap().resolve(10), (3, 10));
        assert_eq!(
            Range::new1(Some(-10), Some(-1)).unwrap().resolve(10),
            (0, 10)
        );
        assert_eq!(Range::new1(Some(4), Some(10)).unwrap().resolve(10), (3, 10));
        assert_eq!(Range::new1(Some(0), Some(10)).unwrap().resolve(10), (0, 10));
        assert_eq!(Range::new1(Some(6), Some(6)).unwrap().resolve(10), (5, 6));
        // end < start
        assert_eq!(Range::new1(Some(6), Some(5)).unwrap().resolve(10), (5, 5));
        assert_eq!(Range::new1(Some(6), Some(4)).unwrap().resolve(10), (5, 5));
    }

    #[test]
    fn test_rng_slice() {
        assert_eq!(
            Range::new(Some(0), Some(10))
                .adjust(true, false)
                .unwrap()
                .resolve(10),
            (0, 10)
        );
        assert_eq!(
            Range::new(Some(3), Some(6))
                .adjust(true, false)
                .unwrap()
                .resolve(10),
            (3, 6)
        );
        assert_eq!(
            Range::new(Some(3), Some(3))
                .adjust(true, false)
                .unwrap()
                .resolve(10),
            (3, 3)
        );
        // start > length -> empty range
        assert_eq!(
            Range::new(Some(12), Some(13))
                .adjust(true, false)
                .unwrap()
                .resolve(10),
            (10, 10)
        );
        // exclusive
        assert_eq!(
            Range::new(Some(0), Some(10))
                .adjust(true, true)
                .unwrap()
                .resolve(10),
            (1, 9)
        );
        assert_eq!(
            Range::new(Some(4), Some(5))
                .adjust(true, true)
                .unwrap()
                .resolve(10),
            (5, 5)
        );
        // open ranges are not changed (ends not trimmed)
        assert_eq!(
            Range::new(None, Some(10))
                .adjust(true, true)
                .unwrap()
                .resolve(10),
            (0, 9)
        );
        assert_eq!(
            Range::new(None, None)
                .adjust(true, true)
                .unwrap()
                .resolve(10),
            (0, 10)
        );
    }
}
