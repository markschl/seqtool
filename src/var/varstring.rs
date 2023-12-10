use std::cell::RefCell;
use std::ops::{Deref, Range};
use std::str;

use bstr::ByteSlice;
use regex::{self, CaptureMatches, Captures};

use crate::error::CliResult;
use crate::helpers::val::TextValue;
use crate::io::Record;
use crate::var;

use super::Func;

lazy_static! {
    // matches { var } or {{ expr }}
    // TODO: but does not handle quoted braces
    static ref WRAPPED_VAR_RE: regex::Regex =
        regex::Regex::new(r"(\{\{(.*?)\}\}|\{(.*?)\})").unwrap();

    // Regex for parsing variables / functions
    // TODO: Regex parsing may be replaced by a more sophisticated parser some time
    static ref VAR_RE: regex::Regex =
        regex::Regex::new(r#"(?x)
        (?:
          "(?:[^"\\]|\\.)+" | '(?:[^'\\]|\\.)+' | `(?:[^`\\]|\\.)+` |   # ignore quoted stuff
          \/(?:[^\/\\]|\\.)+\/[a-z]*   # ignore content of regexes
          |
          \b
          ([a-z_]+)\b  # var/function name
          (?:
            \(       # opening bracket for functions
              (
                (?:\s*("(?:[^"\\]|\\.)+"|'(?:[^'\\]|\\.)+'|[^(),]+)\s*,)*  # args
                   \s*("(?:[^"\\]|\\.)+"|'(?:[^'\\]|\\.)+'|[^(),]+)\s*,?   # last arg (required)
              )
            \)   # closing bracket
          )?
        )
    "#).unwrap();
    static ref ARG_RE: regex::Regex = regex::Regex::new(
        r#"\s*("(?:[^"\\]|\\.)+"|'(?:[^'\\]|\\.)+'|[^(),]+)\s*,?"#
    ).unwrap();
}

#[derive(Debug, Clone)]
pub struct FuncRange(pub Range<usize>, pub Vec<Range<usize>>);

fn parse_re(text: &str, c: Captures<'_>) -> Option<(FuncRange, Func)> {
    c.get(1).and_then(|m| {
        let name = m.as_str().to_string();
        let full_rng = c.get(0).unwrap().range();
        if text.as_bytes().get(full_rng.end + 1) == Some(&b'(') {
            // This means the arguments were not correctly matched by the regex,
            // and the function is thus not valid
            return None;
        }
        let mut rng = FuncRange(full_rng, vec![]);
        let mut args = Vec::new();
        if let Some(arg_group) = c.get(2) {
            // function with args
            for a in ARG_RE.captures_iter(arg_group.as_str()) {
                let m = a.get(1).unwrap();
                args.push(m.as_str().to_string());
                let offset = arg_group.range().start;
                let mut r = m.range();
                r.start += offset;
                r.end += offset;
                rng.1.push(r);
            }
        }
        let f = Func::with_args(name, args);
        Some((rng, f))
    })
}

/// Attempts to find a single variable/function. The text should start with it
/// (if allow_suffix = false, it should be composed of the variable entirely).
/// Otherwise None will be returned.
pub fn parse_single_var(expr: &str, allow_suffix: bool) -> Option<(Func, &str)> {
    let expr = expr.trim();
    VAR_RE.captures(expr).and_then(|c| {
        parse_re(expr, c).and_then(|(rng, func)| {
            let suffix = &expr[rng.0.end..];
            if rng.0.start > 0 || !allow_suffix && !suffix.is_empty() {
                return None;
            }
            Some((func, suffix))
        })
    })
}

pub fn parse_vars(expr: &str) -> VarIter<'_> {
    VarIter {
        expr,
        matches: VAR_RE.captures_iter(expr),
    }
}

pub struct VarIter<'a> {
    expr: &'a str,
    matches: CaptureMatches<'static, 'a>,
}

impl Iterator for VarIter<'_> {
    type Item = (FuncRange, Func);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(c) = self.matches.next() {
                if let Some(item) = parse_re(self.expr, c) {
                    return Some(item);
                }
                continue;
            }
            return None;
        }
    }
}

/// Progressively parses a delimited list of variables, whereby the delimiter
/// (such as a comma) may also be present in the functions themselves.
/// First, the whole item is registered as variable/function. If that fails,
/// a VarString containing mixed text/variables/expressions in braces is assumed.
pub fn register_var_list(
    text: &str,
    delim: &str,
    vars: &mut var::VarBuilder,
    out: &mut Vec<VarString>,
) -> CliResult<()> {
    let mut text = text;
    loop {
        let (s, rest) = VarString::_var_or_composed(text, vars, Some(delim))?;
        out.push(s);
        if rest.is_empty() {
            return Ok(());
        }
        text = &rest[1..];
    }
}

#[derive(Debug, Clone)]
pub enum VarStringPart {
    Text(Vec<u8>),
    Var(usize),
}

impl VarStringPart {
    pub fn get_text(&self) -> Option<&[u8]> {
        match *self {
            VarStringPart::Text(ref t) => Some(t),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VarString {
    parts: Vec<VarStringPart>,
    // consists of only one variable that may also be numeric
    // -> no conversion num -> string -> num necessary
    one_var: bool,
    // used for intermediate storage before conversion to numeric
    num_string: RefCell<TextValue>,
}

impl VarString {
    pub fn from_parts(parts: &[VarStringPart]) -> Self {
        Self {
            parts: parts.to_vec(),
            one_var: parts.len() == 1 && matches!(parts[0], VarStringPart::Var(_)),
            num_string: RefCell::new(TextValue::default()),
        }
    }

    /// Splits the VarString at a given separator.
    /// This is used for parsing slice ranges (start..end)
    /// The implementation is not particularly efficient, but this method is only rarely called
    pub fn split_at(&self, sep: &[u8]) -> Option<(Self, Self)> {
        for i in 0..self.len() {
            if let VarStringPart::Text(ref t) = self.parts[i] {
                if let Some(pos) = t.find(sep) {
                    let mut start = self.parts[..i + 1].to_owned();
                    if let VarStringPart::Text(ref mut t) = start[i] {
                        t.truncate(pos);
                    } else {
                        unreachable!();
                    }
                    let mut end = self.parts[i..].to_owned();
                    if let VarStringPart::Text(ref mut t) = end[0] {
                        *t = t.split_off(pos + sep.len());
                    } else {
                        unreachable!();
                    }
                    return Some((Self::from_parts(&start), Self::from_parts(&end)));
                }
            }
        }
        None
    }

    /// Attempts to parse and register a single variable/function, terminated by the end
    /// of the text or a stop pattern.
    /// Returns None if there is no valid variable/function or there is some residual
    /// text before/after the next variable
    pub fn func<'a>(
        text: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<&str>,
    ) -> Option<(CliResult<Self>, &'a str)> {
        parse_single_var(text, stop.is_some()).and_then(|(func, rest)| {
            let res = vars.register_var(&func).transpose().map(|v| {
                v.map(|(id, _)| Self {
                    parts: vec![(VarStringPart::Var(id))],
                    one_var: true,
                    num_string: RefCell::new(TextValue::default()),
                })
            })?;
            Some((res, rest))
        })
    }

    /// Constructs as VarString that either consists of a single variable
    /// (no {braces} required), or is composed of text, optionally containing
    /// variables/expressions in braces.
    pub fn var_or_composed(text: &str, vars: &mut var::VarBuilder) -> CliResult<Self> {
        Self::_var_or_composed(text, vars, None).map(|(s, _)| s)
    }

    /// Same as var_or_composed, but allows for stopping the search before a given
    /// stop sequence and only constructing a VarString from &text[0..stop].
    pub fn _var_or_composed<'a>(
        text: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<&str>,
    ) -> CliResult<(Self, &'a str)> {
        if let Some((s, rest)) = Self::func(text, vars, stop) {
            return Ok((s?, rest));
        }
        Self::_parse_register(text, vars, stop)
    }

    /// Parses a string containing variables in the form "{varname}"
    /// and/or expressions in the form "{{expression}}"
    pub fn parse_register(expr: &str, vars: &mut var::VarBuilder) -> CliResult<Self> {
        Self::_parse_register(expr, vars, None).map(|(s, _)| s)
    }

    pub fn _parse_register<'a>(
        expr: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<&str>,
    ) -> CliResult<(Self, &'a str)> {
        // println!("parse reg {:?} {:?}", expr, vars);
        let mut parts = vec![];
        let mut prev_pos = 0;
        let mut stop_pos = expr.len();

        for m in WRAPPED_VAR_RE.find_iter(expr) {
            // first check the text before this varible/function match for
            // the delimiter, and finish if found
            let str_before = &expr[prev_pos..m.start()];
            // println!("str before {:?}", str_before);
            if let Some(s) = stop {
                if let Some(pos) = str_before.find(s) {
                    stop_pos = prev_pos + pos;
                    break;
                }
            }
            // the variable regex matches either single or double braces,
            // so we check which ones and proceed correspondingly
            let var = m.as_str();
            let (var_id, _) = if var.starts_with("{{") {
                // matched {{ expression }}
                let expr: &str = &var[2..var.len() - 2];
                let func = Func::expr(expr);
                vars.register_var(&func)?.unwrap()
            } else {
                // matched { variable }
                let var_str = &var[1..var.len() - 1];
                let (func, _) = parse_single_var(var_str, false).ok_or_else(|| {
                    format!(
                        "Invalid variable/function: {}. \
                        Expecting a single variable {{ variable }} or function {{ func(arg) }}. \
                        Advanced expressions (with calculations, etc.) are enclosed in double \
                        braces: {{{{ expression }}}}",
                        var
                    )
                })?;
                vars.register_var(&func)?
                    .ok_or_else(|| format!("Unknown variable/function: {}", func.name))?
            };
            if !str_before.is_empty() {
                parts.push(VarStringPart::Text(str_before.as_bytes().to_owned()));
            }
            parts.push(VarStringPart::Var(var_id));
            // parts.push((str_before.as_bytes().to_owned(), var_id));
            prev_pos = m.end();
        }

        // add the rest
        let mut rest = &expr[prev_pos..stop_pos];
        if let Some(b) = stop {
            // TODO: find() called twice on same text
            // (but this function is rarely called, so not really a problem)
            if let Some(pos) = rest.find(b) {
                stop_pos = pos;
                rest = &rest[..stop_pos];
            }
        }
        if !rest.is_empty() {
            parts.push(VarStringPart::Text(rest.as_bytes().to_owned()));
        }

        Ok((Self::from_parts(&parts), &expr[stop_pos..]))
    }

    // #[inline]
    // fn is_empty(&self) -> bool {
    //     self.get_text().map(|t| t.is_empty()).unwrap_or(false)
    // }

    /// Returns Some(text) if the VarString is composed of text exclusively
    #[inline]
    pub fn get_text(&self) -> Option<&[u8]> {
        if self.parts.len() == 1 {
            self.parts[0].get_text()
        } else {
            None
        }
    }

    #[inline]
    pub fn is_one_var(&self) -> bool {
        self.one_var
    }

    /// Compose the variable string given a filled symbol talbe
    /// Caution: the string is not cleared, any data is appended! clear it by yourself if needed
    #[inline]
    pub fn compose(
        &self,
        out: &mut Vec<u8>,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) {
        for part in &self.parts {
            match part {
                VarStringPart::Text(s) => out.extend_from_slice(s),
                VarStringPart::Var(id) => {
                    table.get(*id).as_text(record, |s| out.extend_from_slice(s));
                }
            }
        }
    }

    /// Converts the variable string to a float, whereby values in single-variable
    /// strings are directly converted (without inefficient writing -> conversion
    /// in case of numeric values)
    #[inline]
    pub fn get_float(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> Option<CliResult<f64>> {
        if self.one_var {
            if let VarStringPart::Var(id) = self.parts[0] {
                return table
                    .get(id)
                    .get_float(record)
                    .map(|res| res.map_err(|e| e.into()));
            }
            unreachable!();
        }
        let mut value = self.num_string.borrow_mut();
        self.compose(value.clear(), table, record);
        if value.len() == 0 {
            return None;
        }
        Some(value.get_float().map_err(|e| e.into()))
    }

    #[inline]
    pub fn get_int(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> Option<CliResult<i64>> {
        if self.one_var {
            if let VarStringPart::Var(id) = self.parts[0] {
                return table
                    .get(id)
                    .get_int(record)
                    .map(|res| res.map_err(|e| e.into()));
            }
            unreachable!();
        }
        let mut value = self.num_string.borrow_mut();
        self.compose(value.clear(), table, record);
        if value.len() == 0 {
            return None;
        }
        Some(value.get_int().map_err(|e| e.into()))
    }
}

impl Deref for VarString {
    type Target = [VarStringPart];

    fn deref(&self) -> &Self::Target {
        &self.parts
    }
}
