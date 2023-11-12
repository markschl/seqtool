use std::cell::RefCell;
use std::ops::Range;
use std::str;

use regex::{self, CaptureMatches, Captures};

use crate::error::CliResult;
use crate::io::Record;
use crate::var;

use super::Func;

lazy_static! {
    // matches { var } or {{ expr }}
    // (but does not handle quoted braces)
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
/// (such as a comma) may also be present in the functions themselves
pub fn register_var_list(
    text: &str,
    delim: char,
    vars: &mut var::VarBuilder,
    out: &mut Vec<VarString>,
) -> CliResult<()> {
    let mut text = text;
    loop {
        // println!("parse {:?}", &text);
        let (s, rest) = VarString::_var_or_composed(text, vars, Some(delim))?;
        out.push(s);
        if rest.is_empty() {
            return Ok(());
        }
        text = &rest[1..];
    }
}

#[derive(Debug, Default)]
pub struct VarString {
    // (String before, var_id)
    parts: Vec<(Vec<u8>, usize)>,
    rest: Vec<u8>,
    // consists of only one variable that may also be numeric
    // -> no conversion num -> string -> num necessary
    one_var: bool,
    // used for intermediate storage before conversion to numeric
    num_string: RefCell<Vec<u8>>,
}

impl VarString {
    pub fn func<'a>(
        text: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<char>,
    ) -> Option<(CliResult<Self>, &'a str)> {
        parse_single_var(text, stop.is_some()).and_then(|(func, rest)| {
            // println!("func {:?} {:?} {:?} {:?}", func, rest, rest.chars().nth(0), stop);
            if rest.chars().nth(0).or(stop) != stop {
                return None;
            }
            let res = vars.register_var(&func).transpose().map(|v| {
                v.map(|(id, _)| Self {
                    parts: vec![(vec![], id)],
                    rest: vec![],
                    one_var: true,
                    num_string: RefCell::new(vec![]),
                })
            })?;
            Some((res, rest))
        })
    }

    /// Constructs as VarString that either consists of a single variable
    /// (no {braces} required). If the given string is not a valid variable,
    /// we assume that it is a string containing variables/expressions
    /// with braces.
    pub fn var_or_composed(text: &str, vars: &mut var::VarBuilder) -> CliResult<Self> {
        Self::_var_or_composed(text, vars, None).map(|(s, _)| s)
    }

    pub fn _var_or_composed<'a>(
        text: &'a str,
        vars: &mut var::VarBuilder,
        stop: Option<char>,
    ) -> CliResult<(Self, &'a str)> {
        if let Some((s, rest)) = Self::func(text, vars, stop) {
            // println!("voc found {:?}", (&s, rest));
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
        stop: Option<char>,
    ) -> CliResult<(Self, &'a str)> {
        // println!("parse reg {:?} {:?}", expr, vars);
        let mut outvars = vec![];
        let mut prev_pos = 0;
        let mut stop_pos = expr.len();

        for m in WRAPPED_VAR_RE.find_iter(expr) {
            let str_before = &expr[prev_pos..m.start()];
            // println!("str before {:?}", str_before);
            if let Some(b) = stop {
                if let Some(pos) = str_before.find(b) {
                    stop_pos = prev_pos + pos;
                    break;
                }
            }
            let var = m.as_str();
            let (var_id, _) = if var.starts_with("{{") {
                // {{ expression }}
                let expr: &str = &var[2..var.len() - 2];
                let func = Func::expr(expr);
                vars.register_var(&func)?.unwrap()
            } else {
                // { variable }
                let var_str = &var[1..var.len() - 1];
                let (func, _) = parse_single_var(var_str, false)
                    .ok_or_else(|| format!("Invalid variable/function: {}. Expecting a single variable/function: {{ func(arg) }}. More advanced expressions (with calculations, etc.) are specified like this: {{{{ expr }}}}", var))?;
                vars.register_var(&func)?
                    .ok_or_else(|| format!("Unknown variable: {}", func.name))?
            };
            outvars.push((str_before.as_bytes().to_owned(), var_id));
            prev_pos = m.end();
        }

        // add the rest
        let mut rest = &expr[prev_pos..stop_pos];
        if let Some(b) = stop {
            // TODO: find() called twice on same text (but this function is rarely called)
            if let Some(pos) = rest.find(b) {
                stop_pos = pos;
                rest = &rest[..stop_pos];
            }
        }
        // dbg!((&outvars, rest));

        let one_var = outvars.len() == 1 && outvars[0].0.is_empty() && rest.is_empty();

        let s = VarString {
            parts: outvars,
            rest: rest.as_bytes().to_owned(),
            one_var,
            num_string: RefCell::new(vec![]),
        };
        Ok((s, &expr[stop_pos..]))
    }

    /// Caution: the string is not cleared, any data is appended! clear it by yourself if needed
    #[inline]
    pub fn compose(
        &self,
        out: &mut Vec<u8>,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) {
        for &(ref str_before, id) in &self.parts {
            out.extend_from_slice(str_before);
            table.get(id).as_text(record, |s| out.extend_from_slice(s));
        }
        out.extend_from_slice(&self.rest);
    }

    #[inline]
    pub fn get_float(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> Option<CliResult<f64>> {
        if self.one_var {
            return table
                .get(self.parts[0].1)
                .get_float(record)
                .map(|res| res.map_err(|e| e.into()));
        }
        let mut string = self.num_string.borrow_mut();
        string.clear();
        self.compose(&mut string, table, record);
        if string.len() == 0 {
            return None;
        }
        Some(
            str::from_utf8(&string)
                .map_err(From::from)
                .and_then(|s| s.parse().map_err(From::from)),
        )
    }

    #[inline]
    pub fn get_int(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> Option<CliResult<i64>> {
        if self.one_var {
            return table
                .get(self.parts[0].1)
                .get_int(record)
                .map(|res| res.map_err(|e| e.into()));
        }
        let mut string = self.num_string.borrow_mut();
        string.clear();
        self.compose(&mut string, table, record);
        if string.len() == 0 {
            return None;
        }
        Some(atoi::atoi(&string).ok_or_else(|| {
            format!(
                "Could not parse '{}' as integer.",
                String::from_utf8_lossy(&string)
            )
            .into()
        }))
    }
}
