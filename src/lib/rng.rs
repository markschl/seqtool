/// Methods for working with variable ranges
use std::cmp::max;
// use attr;
// use attr::KeyGetter;
use error::CliResult;
use lib::inner_result::MapRes;
use lib::util;
use var;
use var::varstring::VarString;

/// Represents a range bound integer stored either directly or in a `VarString`
/// that is evaluated later with `RngBound::value()`.
#[derive(Debug)]
pub enum RngBound {
    Number(isize),
    Expr(VarString),
}

impl RngBound {
    pub fn from_str(value: &str, vars: &mut var::Vars) -> CliResult<RngBound> {
        Ok(match value.parse::<isize>() {
            Ok(n) => RngBound::Number(n),
            Err(_) => {
                let expr = vars.build(|b| VarString::var_or_composed(value, b))?;
                RngBound::Expr(expr)
            }
        })
    }

    pub fn value(&self, symbols: &var::symbols::Table) -> CliResult<isize> {
        Ok(match *self {
            RngBound::Number(n) => n,
            RngBound::Expr(ref e) => {
                e.get_int(symbols)?
                    .ok_or("Range bound results in empty string.")? as isize
            }
        })
    }
}

/// Represents a range that is either stored directly or evaluated
/// later
#[derive(Debug)]
pub enum VarRange {
    /// range (..) notation already found in input text
    Split(Option<RngBound>, Option<RngBound>),
    /// range notation will be present in composed `VarString`
    Full(VarString, Vec<u8>),
}

impl VarRange {
    pub fn from_str(s: &str, vars: &mut var::Vars) -> CliResult<VarRange> {
        if let Ok((start, end)) = util::parse_range_str(s) {
            Ok(VarRange::Split(
                start.map_res(|s| RngBound::from_str(s, vars))?,
                end.map_res(|e| RngBound::from_str(e, vars))?,
            ))
        } else {
            let varstring = vars.build(|b| VarString::var_or_composed(s, b))?;
            Ok(VarRange::Full(varstring, vec![]))
        }
    }

    pub fn get(
        &mut self,
        length: usize,
        rng0: bool,
        exclusive: bool,
        symbols: &var::symbols::Table,
    ) -> CliResult<(usize, usize)> {
        Ok(match *self {
            VarRange::Split(ref start, ref end) => Range::new(
                start.as_ref().map_res(|s| s.value(symbols))?,
                end.as_ref().map_res(|e| e.value(symbols))?,
                length,
                rng0,
            )?.get(exclusive),
            VarRange::Full(ref varstring, ref mut val) => {
                val.clear();
                varstring.compose(val, symbols);
                // "unnecessary" UTF-8 conversion, however it would be complicated
                // because there is no integer parsing from byte slices in the std. library
                let s = ::std::str::from_utf8(val)?;
                let (start, end) = util::parse_range(s)?;
                Range::new(start, end, length, rng0)?.get(exclusive)
            }
        })
    }
}

#[derive(Debug)]
pub enum VarRangesType {
    Split(Vec<VarRange>),
    Full(VarString),
}

#[derive(Debug)]
pub struct VarRanges {
    ty: VarRangesType,
    out: Vec<(usize, usize)>,
    val: Vec<u8>,
}

impl VarRanges {
    pub fn from_str(s: &str, vars: &mut var::Vars) -> CliResult<VarRanges> {
        let ranges: Vec<_> = s.split(',').collect();
        let ty = if ranges.len() == 1 {
            let varstring = vars.build(|b| VarString::var_or_composed(s, b))?;
            VarRangesType::Full(varstring)
        } else {
            VarRangesType::Split(
                ranges
                    .into_iter()
                    .map(|r| VarRange::from_str(r, vars))
                    .collect::<CliResult<_>>()?,
            )
        };
        Ok(VarRanges {
            ty: ty,
            out: vec![],
            val: vec![],
        })
    }

    pub fn get(
        &mut self,
        length: usize,
        rng0: bool,
        exclusive: bool,
        symbols: &var::symbols::Table,
    ) -> CliResult<&[(usize, usize)]> {
        self.out.clear();
        match self.ty {
            VarRangesType::Split(ref mut rng) => for r in rng {
                self.out.push(r.get(length, rng0, exclusive, symbols)?);
            },
            VarRangesType::Full(ref varstring) => {
                self.val.clear();
                varstring.compose(&mut self.val, symbols);
                let s = ::std::str::from_utf8(&self.val)?;
                for r in s.split(',') {
                    let (start, end) = util::parse_range(r)?;
                    let r = Range::new(start, end, length, rng0)?.get(exclusive);
                    self.out.push(r);
                }
            }
        }
        Ok(&self.out)
    }
}

// locate range with negative numbers on sequence once the length is known
// requires and returns 1-based coordinates according to R's start:end range notation

// 0-based range
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Range(usize, usize);

impl Range {
    pub fn new(
        start: Option<isize>,
        end: Option<isize>,
        length: usize,
        rng0: bool,
    ) -> Result<Range, &'static str> {
        if rng0 {
            let start = start.unwrap_or(0);
            let end = end.unwrap_or(length as isize);
            if start < 0 || end < 0 {
                return Err("0-based ranges must not contain negative numbers.");
            }
            Ok(Self::from_rng0(start as usize, end as usize, length))
        } else {
            let start = start.unwrap_or(1);
            let end = end.unwrap_or(-1);
            Ok(Self::from_rng1(start, end, length))
        }
    }

    // takes *1-based* inclusive range as input with optional negative numbers and seq. length
    // to normalize with
    pub fn from_rng1(start: isize, end: isize, length: usize) -> Range {
        let s = Self::_cnv_neg(start, length);
        let e = Self::_cnv_neg(end, length);
        Self::from_rng0(s - 1, e, length)
    }

    pub fn from_rng0(start: usize, end: usize, length: usize) -> Range {
        let mut end = end;
        if start > end {
            end = start;
        }
        if end > length {
            end = length;
        }
        Range(start, end)
    }

    // 1-based (with negative) -> 1-based (positive)
    pub fn _cnv_neg(pos: isize, length: usize) -> usize {
        let pos = if pos < 0 {
            length as isize + pos + 1
        } else {
            pos
        };
        max(pos, 1) as usize
    }

    pub fn get(&self, exclusive: bool) -> (usize, usize) {
        let mut start = self.0;
        let mut end = self.1;
        if exclusive && start < end {
            start += 1;
            if end > 0 && start < end {
                end -= 1;
            }
        }
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng() {
        // 0-based range as input
        assert_eq!(Range::from_rng0(3, 10, 10), Range(3, 10));
        assert_eq!(Range::from_rng0(5, 4, 10), Range(5, 5));
        // 1-based range as input
        assert_eq!(Range::from_rng1(4, 10, 10), Range(3, 10));
        assert_eq!(Range::from_rng1(4, -1, 10), Range(3, 10));
        assert_eq!(Range::from_rng1(-10, -1, 10), Range(0, 10));
        assert_eq!(Range::from_rng1(4, 11, 10), Range(3, 10));
        assert_eq!(Range::from_rng1(0, 11, 10), Range(0, 10));
        assert_eq!(Range::from_rng1(6, 6, 10), Range(5, 6));
        assert_eq!(Range::from_rng1(6, 5, 10), Range(5, 5));
        assert_eq!(Range::from_rng1(6, 4, 10), Range(5, 5));
    }

    #[test]
    fn test_rng_slice() {
        assert_eq!(Range(0, 10).get(false), (0, 10));
        assert_eq!(Range(3, 6).get(false), (3, 6));
        assert_eq!(Range(3, 3).get(false), (3, 3));
        // exclusive
        assert_eq!(Range(0, 10).get(true), (1, 9));
        assert_eq!(Range(4, 5).get(true), (5, 5));
    }
}
