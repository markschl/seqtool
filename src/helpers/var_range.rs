use std::str::FromStr;

use crate::error::CliResult;
use crate::io::Record;
use crate::var::{
    symbols::SymbolTable,
    varstring::{register_var_list, VarString},
    VarBuilder,
};

use super::rng::Range;

/// Represents a range bound integer stored either directly or in a `VarString`
/// that is evaluated later with `RngBound::value()`.
#[derive(Debug, Clone)]
pub enum RngBound {
    Number(isize),
    Expr(VarString),
}

impl RngBound {
    pub fn from_varstring(vs: VarString) -> CliResult<Option<RngBound>> {
        if let Some(text) = vs.get_text() {
            if text.is_empty() {
                return Ok(None);
            }
            if let Some(bound) = atoi::atoi(text) {
                return Ok(Some(RngBound::Number(bound)));
            }
        }
        Ok(Some(RngBound::Expr(vs)))
    }

    pub fn value(
        &self,
        symbols: &SymbolTable,
        record: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) -> CliResult<isize> {
        Ok(match *self {
            RngBound::Number(n) => n,
            RngBound::Expr(ref e) => {
                e.get_int(symbols, record, text_buf)?
                    .ok_or("Range bound results in empty string.")? as isize
            }
        })
    }
}

/// Represents a range that is either stored directly or evaluated
/// later
#[derive(Debug, Clone)]
pub enum VarRange {
    /// range (..) notation already found in input text
    Split(Option<RngBound>, Option<RngBound>),
    /// range notation will be present in composed `VarString`
    /// The Vec<u8> is a reusable allocation.
    Full(VarString, Vec<u8>),
}

impl VarRange {
    /// Obtain range from start..end or {start_var}..{end_var} or
    /// {range_var}, whose value should be a valid range.
    /// In theory, more complicated compositions are possible, but they will
    /// rarely result in useful/valid ranges.
    pub fn from_varstring(vs: VarString) -> CliResult<VarRange> {
        if vs.len() == 1 && vs.is_one_var() {
            return Ok(VarRange::Full(vs, Vec::with_capacity(20)));
        }
        if let Some((start, end)) = vs.split_at(b"..") {
            return Ok(VarRange::Split(
                RngBound::from_varstring(start)?,
                RngBound::from_varstring(end)?,
            ));
        }
        fail!("Invalid variable range. Valid are 'start..end', 'start..', '..end' or '..'")
    }

    /// Replace variables to obtain the actual range
    pub fn resolve(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) -> CliResult<Range> {
        Ok(match *self {
            VarRange::Split(ref mut start, ref mut end) => Range::new(
                start
                    .as_ref()
                    .map(|s| s.value(symbols, record, text_buf))
                    .transpose()?,
                end.as_ref()
                    .map(|e| e.value(symbols, record, text_buf))
                    .transpose()?,
            ),
            VarRange::Full(ref varstring, ref mut val) => {
                val.clear();
                varstring.compose(val, symbols, record)?;
                // TODO: unnecessary UTF-8 conversion -> investigate range parsing from byte slices (FromStr right now)
                let s = std::str::from_utf8(val)?;
                Range::from_str(s)?
            }
        })
    }
}

/// Represents a list of variable ranges
#[derive(Debug)]
pub enum VarRangesType {
    Split(Vec<VarRange>),
    Full(VarString),
}

/// Represents a list of variable ranges, whereby the evaluation results
/// are cached.
#[derive(Debug)]
pub struct VarRanges {
    ty: VarRangesType,
    out: Vec<Range>,
    val: Vec<u8>,
}

impl VarRanges {
    pub fn from_str(s: &str, var_builder: &mut VarBuilder) -> CliResult<VarRanges> {
        // first, we collect all comma-delimited parts, registering any variables
        let mut parts = vec![];
        register_var_list(s.trim(), var_builder, &mut parts, true)?;
        // then, we parse all ranges
        let mut ranges: Vec<VarRange> = parts
            .into_iter()
            .map(VarRange::from_varstring)
            .collect::<CliResult<_>>()?;
        // single-variable strings may hold a range list (not only a single range)
        let mut ty = VarRangesType::Split(ranges.clone());
        if ranges.len() == 1 {
            if let VarRange::Full(vs, _) = ranges.drain(..).next().unwrap() {
                if vs.is_one_var() {
                    ty = VarRangesType::Full(vs)
                }
            }
        }
        Ok(VarRanges {
            ty,
            out: vec![],
            val: vec![],
        })
    }

    pub fn resolve(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) -> CliResult<&[Range]> {
        self.out.clear();
        match self.ty {
            VarRangesType::Split(ref mut rng) => {
                for r in rng {
                    self.out.push(r.resolve(symbols, record, text_buf)?);
                }
            }
            VarRangesType::Full(ref varstring) => {
                self.val.clear();
                varstring.compose(&mut self.val, symbols, record)?;
                let s = std::str::from_utf8(&self.val)?;
                for r in s.split(',') {
                    self.out.push(Range::from_str(r)?);
                }
            }
        }
        Ok(&self.out)
    }
}
