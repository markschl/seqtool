use std::str::FromStr;

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

    pub fn adjust(mut self, range0: bool, exclusive: bool) -> Result<Self, String> {
        if !range0 {
            // input is 1-based -> convert to 0-based coordinates,
            // which are used internally:
            // start coordinate - 1
            if let Some(s) = self.start.as_mut() {
                if *s >= 1 {
                    *s -= 1;
                }
            }
            // negative coordinates: end coordinate + 1
            if self.end == Some(-1) {
                self.end = None;
            } else if let Some(e) = self.end.as_mut() {
                if *e <= -2 {
                    *e += 1;
                }
            }
        }
        // TODO: check
        if exclusive {
            self.start = Some(self.start.unwrap_or(0) + 1);
            self.end = Some(
                self.end
                    .map(|e| if e != 0 { e - 1 } else { 0 })
                    .unwrap_or(-1),
            );
        }
        Ok(self)
    }

    pub fn obtain(&self, length: usize) -> (usize, usize) {
        // resolve negative bounds
        let mut start = self.start.unwrap_or(0);
        if start < 0 {
            start = (length as isize + start).max(0);
        }
        let mut end = self.end.unwrap_or(length as isize);
        if end < 0 {
            end = (length as isize + end).max(0);
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

impl FromStr for Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, "..").map(|s| s.trim());
        let start = parts.next().unwrap();
        if let Some(end) = parts.next() {
            if let None = parts.next() {
                let start = if start.is_empty() {
                    None
                } else {
                    Some(
                        start
                            .parse()
                            .map_err(|_| format!("Invalid range start: '{}'", start))?,
                    )
                };
                let end = if end.is_empty() {
                    None
                } else {
                    Some(
                        end.parse()
                            .map_err(|_| format!("Invalid range end: '{}'", end))?,
                    )
                };
                return Ok(Self { start, end });
            }
        }
        Err(format!(
            "Invalid range: '{}'. Possible notations: 'start..end', 'start..', '..end', or '..'",
            s
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng() {
        // 0-based range as input
        assert_eq!(Range::new0(Some(3), Some(10)).obtain(10), (3, 10));
        // start > end -> range adjusted
        assert_eq!(Range::new0(Some(5), Some(4)).obtain(10), (5, 5));
        // 1-based range as input
        assert_eq!(Range::new1(Some(4), Some(10)).unwrap().obtain(10), (3, 10));
        assert_eq!(Range::new1(Some(4), Some(-1)).unwrap().obtain(10), (3, 10));
        assert_eq!(
            Range::new1(Some(-10), Some(-1)).unwrap().obtain(10),
            (0, 10)
        );
        assert_eq!(Range::new1(Some(4), Some(10)).unwrap().obtain(10), (3, 10));
        assert_eq!(Range::new1(Some(0), Some(10)).unwrap().obtain(10), (0, 10));
        assert_eq!(Range::new1(Some(6), Some(6)).unwrap().obtain(10), (5, 6));
        // end < start
        assert_eq!(Range::new1(Some(6), Some(5)).unwrap().obtain(10), (5, 5));
        assert_eq!(Range::new1(Some(6), Some(4)).unwrap().obtain(10), (5, 5));
    }

    #[test]
    fn test_rng_slice() {
        assert_eq!(
            Range::new(Some(0), Some(10))
                .adjust(true, false)
                .unwrap()
                .obtain(10),
            (0, 10)
        );
        assert_eq!(
            Range::new(Some(3), Some(6))
                .adjust(true, false)
                .unwrap()
                .obtain(10),
            (3, 6)
        );
        assert_eq!(
            Range::new(Some(3), Some(3))
                .adjust(true, false)
                .unwrap()
                .obtain(10),
            (3, 3)
        );
        // exclusive
        assert_eq!(
            Range::new(Some(0), Some(10))
                .adjust(true, true)
                .unwrap()
                .obtain(10),
            (1, 9)
        );
        assert_eq!(
            Range::new(Some(4), Some(5))
                .adjust(true, true)
                .unwrap()
                .obtain(10),
            (5, 5)
        );
    }
}
