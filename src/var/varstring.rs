use std::borrow::Cow;
use std::ops::{Deref, Range};
use std::{io, str};

use bstr::ByteSlice;
use ordered_float::OrderedFloat;
cfg_if::cfg_if! {
    // The fully blown regex implementation is only used if it is effectively
    // used by some command. Otherwise, we just use regex-lite.
    if #[cfg(all(feature = "regex-fast", any(feature = "all_commands", feature = "find", feature = "replace")))] {
        use regex as regex_mod;
    } else {
        use regex_lite as regex_mod;
    }
}
use regex_mod::{CaptureMatches, Captures, Regex};

use crate::error::CliResult;
use crate::helpers::util::{text_to_float, text_to_int};
use crate::helpers::value::SimpleValue;
use crate::io::Record;
use crate::var;

use super::symbols::{Value, VarType};
use super::Func;

lazy_static! {
    // matches { var } or {{ expr }}
    // TODO: but does not handle quoted braces
    static ref FUNC_BRACES_RE: Regex =
        Regex::new(r"(\{\{(.*?)\}\}|\{(.*?)\})").unwrap();

    // Regex for matching variables / functions
    // TODO: Regex parsing may be replaced by a more sophisticated parser some time
    static ref VAR_FUNC_RE: Regex =
        Regex::new(r#"(?x)
        (?:
          "(?:[^"\\]|\\.)+" | '(?:[^'\\]|\\.)+' | `(?:[^`\\]|\\.)+`   # ignore quoted ("'`) stuff
          |
          (?-u:\b)
          ([a-z_]+) (?-u:\b)  # var/function name
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
    static ref ARG_RE: Regex = Regex::new(
        r#"\s*("(?:[^"\\]|\\.)+"|'(?:[^'\\]|\\.)+'|[^(),]+)\s*,?"#
    ).unwrap();
}

/// Struct containing positional information of a `Func` in a text:
/// The first range is the full range, the second is the range of the arguments.
#[derive(Debug, Clone)]
pub struct FuncRange {
    pub full: Range<usize>,
    pub name: Range<usize>,
    pub args: Vec<Range<usize>>,
}

fn _parse_re_captures(text: &str, c: Captures<'_>) -> Option<(FuncRange, Func)> {
    c.get(1).and_then(|m| {
        let name = m.as_str().to_string();
        let full_rng = c.get(0).unwrap().range();
        if text.as_bytes().get(full_rng.end + 1) == Some(&b'(') {
            // This means the arguments were not correctly matched by the regex,
            // and the function is thus not valid
            return None;
        }
        let mut rng = FuncRange {
            full: full_rng,
            name: m.range(),
            args: vec![],
        };
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
                rng.args.push(r);
            }
        }
        let f = Func::with_args(name, args);
        Some((rng, f))
    })
}

/// Parses all variables/functions (according to 'seqtool' syntax) that
/// are found at any place in the text.
pub fn parse_vars(expr: &str) -> VarIter<'_> {
    VarIter {
        expr,
        matches: VAR_FUNC_RE.captures_iter(expr),
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
                if let Some(item) = _parse_re_captures(self.expr, c) {
                    return Some(item);
                }
                continue;
            }
            return None;
        }
    }
}

/// Attempts to find a single variable/function at the *start of the input text.*
/// Whitespace is ignored.
/// This is different from `parse_vars()`, which returns all variables/functions
/// matched at any place in the text.
/// Returns the `Func` and the remaining text or `None` if no var/func found.
pub fn parse_var(expr: &str) -> Option<(Func, &str)> {
    let expr = expr.trim();
    parse_vars(expr).next().and_then(|(rng, func)| {
        if rng.full.start == 0 {
            return Some((func, &expr[rng.full.end..]));
        }
        None
    })
}

/// Attempts to find a single variable/function that matches the *whole* text.
/// Whitespace is ignored.
pub fn parse_single_var(expr: &str) -> Option<Func> {
    parse_var(expr).and_then(|(f, rest)| {
        if !rest.is_empty() {
            return None;
        }
        Some(f)
    })
}

/// Progressively parses a delimited list of variables/functions, whereby the delimiter
/// (such as a comma) may also be present in the functions themselves.
/// First, the whole item is registered as variable/function. If that fails,
/// a VarString containing mixed text/variables/expressions in braces is assumed.
/// If `allow_single_var` is true, a the whole text is be interpreted
/// as variable (if registration succeeeds), otherwise as text.
pub fn register_var_list(
    text: &str,
    delim: char,
    vars: &mut var::VarBuilder,
    out: &mut Vec<VarString>,
    allow_single_var: bool,
) -> CliResult<()> {
    let mut text = text;
    while !text.is_empty() {
        let (s, _, rest) =
            VarString::parse_register_until(text, vars, Some(delim), allow_single_var)?;
        out.push(s);
        text = rest;
    }
    Ok(())
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

/// This type represents text, which can contain variables and/or expressions.
/// It implements `Deref<Target=[VarStringPart]>` for easy access to the individual
/// components.
#[derive(Debug, Clone, Default)]
pub struct VarString {
    parts: Vec<VarStringPart>,
    // Variable ID if consists of only one variable that may also be numeric,
    // and thus, type conversion to numeric will be simpler/unnecessary
    one_var: Option<usize>,
}

impl VarString {
    pub fn from_parts(parts: &[VarStringPart]) -> Self {
        debug_assert!(!parts.is_empty());
        Self {
            parts: parts.to_vec(),
            one_var: match parts[0] {
                VarStringPart::Var(id) if parts.len() == 1 => Some(id),
                _ => None,
            },
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

    /// Parses a string containing variables in the form "{varname}"
    /// and/or expressions in the form "{{expression}}"
    /// If `allow_single_var` is true, a the whole text is be interpreted
    /// as variable (if registration succeeeds), otherwise as text.
    pub fn parse_register(
        expr: &str,
        vars: &mut var::VarBuilder,
        allow_single_var: bool,
    ) -> CliResult<(Self, Option<VarType>)> {
        Self::parse_register_until(expr, vars, None, allow_single_var).map(|(s, ty, _)| (s, ty))
    }

    /// Main function for parsing VarStrings from text.
    /// The optional stop pattern can be used to parse delimited lists.
    /// If `allow_single_var` is true, a the whole text (until the stop char)
    /// is be interpreted as variable (if registration succeeeds), otherwise
    /// as text.
    pub fn parse_register_until<'a>(
        text: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<char>,
        allow_single_var: bool,
    ) -> CliResult<(Self, Option<VarType>, &'a str)> {
        // println!("parse reg {:?} {:?}", text, vars);

        // If `allow_single_var`, try registering the whole text content (until stop)
        // as variable/function
        if allow_single_var {
            if let Some((func, rest)) = parse_var(text) {
                let (one_var, rest) = if rest.is_empty() {
                    (true, "")
                } else {
                    stop.map(|c| (rest.starts_with(c), &rest[1..]))
                        .unwrap_or((false, rest))
                };
                if one_var {
                    if let Some((var_id, ty, _)) = vars.register_var(&func)? {
                        return Ok((Self::from_parts(&[VarStringPart::Var(var_id)]), ty, rest));
                    }
                }
            }
        }

        // otherwise, parse different components
        let mut parts = vec![];
        let mut prev_pos = 0;
        let mut vtype = Some(VarType::Text);

        // Search for variables/functions
        // (the stop char is only searched afterwards, since function args
        // may also contain the stop)
        for m in FUNC_BRACES_RE.find_iter(text) {
            // Once a var is found, further check the preceding text for
            // the stop, and finish if found
            let str_before = &text[prev_pos..m.start()];
            if let Some(s) = stop {
                if str_before.contains(s) {
                    break;
                }
            }
            // the variable regex matches either single or double braces,
            // so we check which ones and proceed correspondingly
            let var = m.as_str();
            let (var_id, ty, _) = if var.starts_with("{{") && cfg!(feature = "expr") {
                // matched {{ expression }}
                let expr: &str = &var[2..var.len() - 2];
                let func = Func::expr(expr);
                vars.register_var(&func)?.unwrap()
            } else {
                // matched { variable } or { function(arg,...) }
                let func_str = &var[1..var.len() - 1];
                let func = parse_single_var(func_str).ok_or_else(|| {
                    let extra = "Advanced expressions (with calculations, etc.) \
                    are enclosed in double braces: {{ expression }}";
                    format!(
                        "Invalid variable/function: {}. \
                        Expecting a single variable {{ variable }} or function {{ func(arg) }}.{}",
                        var,
                        if cfg!(feature = "expr") { extra } else { "" }
                    )
                })?;
                vars.register_var(&func)?
                    .ok_or_else(|| format!("Unknown variable/function: {}", func.name))?
            };
            if !str_before.is_empty() {
                parts.push(VarStringPart::Text(str_before.as_bytes().to_owned()));
            } else if parts.is_empty() {
                // first variable without any text before it,
                // so assign the type of this varialble
                // (may be switched back to text later if more parts are added)
                vtype = ty;
            }
            parts.push(VarStringPart::Var(var_id));
            prev_pos = m.end();
        }

        // This part handles all text that does not contain variables/functions
        // (that is, either the whole text or the remaining part after vars/funcs)
        let text = &text[prev_pos..];

        // Check for stop sequence
        let (stop_pos, rest_start) = stop
            .and_then(|b| text.find(b).map(|p| (p, p + 1)))
            .unwrap_or((text.len(), text.len()));
        let trimmed = &text[..stop_pos];

        // add text component
        if !trimmed.is_empty() || parts.is_empty() {
            parts.push(VarStringPart::Text(trimmed.as_bytes().to_owned()));
        }

        // set type [back] to text if VarString is composed of multiple parts
        // (e.g. concatenated numeric variables would result in text)
        if parts.len() > 1 {
            vtype = Some(VarType::Text);
        }
        // println!("--> out: {:?} / {:?}, rest: {}", parts, vtype, &text[rest_start..]);
        Ok((Self::from_parts(&parts), vtype, &text[rest_start..]))
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
        self.one_var.is_some()
    }

    /// Compose the variable string given a filled symbol talbe
    /// Caution: the string is not cleared, any data is appended! clear it by yourself if needed
    #[inline]
    pub fn compose<W: io::Write + ?Sized>(
        &self,
        out: &mut W,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> CliResult<()> {
        if let Some(id) = self.one_var {
            table
                .get(id)
                .inner()
                .map(|v| {
                    v.as_text(record, |s| {
                        out.write_all(s)?;
                        Ok(())
                    })
                })
                .transpose()?;
        } else {
            for part in &self.parts {
                match part {
                    VarStringPart::Text(s) => {
                        out.write_all(s)?;
                    }
                    VarStringPart::Var(id) => {
                        table
                            .get(*id)
                            .inner()
                            .map(|v| {
                                v.as_text(record, |s| {
                                    out.write_all(s)?;
                                    Ok(())
                                })
                            })
                            .transpose()?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a value if the VarString contains only a single variable
    /// without any preceding/following text
    #[inline]
    pub fn get_one_value<'a>(&self, table: &'a var::symbols::SymbolTable) -> Option<&'a Value> {
        self.one_var.and_then(|id| table.get(id).inner())
    }

    /// Obtains the integer value
    #[inline]
    pub fn get_int(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) -> CliResult<Option<i64>> {
        if let Some(id) = self.one_var {
            return table
                .get(id)
                .inner()
                .map(|v| v.get_int(record).map_err(|e| e.into()))
                .transpose();
        }
        text_buf.clear();
        self.compose(text_buf, table, record)?;
        if text_buf.is_empty() {
            return Ok(None);
        }
        Ok(Some(text_to_int(text_buf)?))
    }

    /// Returns a SimpleValue (text/numeric/none).
    /// Requires an extra 'text_buf', which allows retaining text allocations
    /// and must always be a `SimpleValue::Text`.
    #[inline]
    pub fn get_simple<'a>(
        &self,
        text_buf: &'a mut SimpleValue,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
        force_numeric: bool,
    ) -> CliResult<Cow<'a, SimpleValue>> {
        if let Some(v) = self.get_one_value(table) {
            if v.is_numeric() {
                let val = v.get_float(record)?;
                return Ok(Cow::Owned(SimpleValue::Number(OrderedFloat(val))));
            }
        }
        let text = match text_buf {
            SimpleValue::Text(t) => t,
            _ => panic!(),
        };
        text.clear();
        self.compose(text, table, record)?;

        if !text.is_empty() {
            if !force_numeric {
                text.shrink_to_fit();
                Ok(Cow::Borrowed(&*text_buf))
            } else {
                let val = text_to_float(text)?;
                Ok(Cow::Owned(SimpleValue::Number(OrderedFloat(val))))
            }
        } else {
            Ok(Cow::Owned(SimpleValue::None))
        }
    }
}

impl Deref for VarString {
    type Target = [VarStringPart];

    fn deref(&self) -> &Self::Target {
        &self.parts
    }
}
