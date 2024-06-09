use std::ops::Deref;
use std::{io, mem, str};

use memchr::memmem;
use var_provider::VarType;

use crate::helpers::{number::parse_int, value::SimpleValue, NA};
use crate::io::Record;
use crate::var;

use super::parser::{parse_varstring, parse_varstring_list, ParsedVarStringSegment};
use super::VarBuilder;

/// Parses a comma delimited list of variables/functions, whereby the
/// delimiter is only searched in text inbetween vars/functions.
/// If `raw_var` is true, the parser will attempt to find and register variables/functions
/// **without** braces around them, falling back to text mode if registration fails.
pub fn register_var_list(
    text: &str,
    builder: &mut VarBuilder,
    out: &mut Vec<VarString>,
    raw_var: bool,
    require_vars: bool,
) -> Result<(), String> {
    for frags in parse_varstring_list(text, raw_var)? {
        let (vs, _) = VarString::register_parsed(&frags, builder)?;
        if require_vars && vs.len() == 1 {
            if let VarStringSegment::Text(t) = &vs[0] {
                return fail!(
                    "The following string contains no variables/functions: '{}' \
                    Is it possible that you misspelled the variable/function name? \
                    See also st <command> --help-vars.",
                    String::from_utf8_lossy(t)
                );
            }
        }
        out.push(vs);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarStringSegment {
    Text(Vec<u8>),
    Var(usize),
}

impl VarStringSegment {
    pub fn get_text(&self) -> Option<&[u8]> {
        match *self {
            VarStringSegment::Text(ref t) => Some(t),
            _ => None,
        }
    }
}

/// This type represents text, which can contain variables and/or expressions.
/// It implements `Deref<Target=[VarStringPart]>` for easy access to the individual
/// components.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarString {
    parts: Vec<VarStringSegment>,
    // Variable ID if consists of only one variable that may also be numeric,
    // and thus, type conversion to numeric will be simpler/unnecessary
    one_var: Option<usize>,
}

impl VarString {
    pub fn from_segments(parts: &[VarStringSegment]) -> Self {
        Self {
            parts: parts.to_vec(),
            one_var: match parts.first() {
                Some(VarStringSegment::Var(id)) if parts.len() == 1 => Some(*id),
                _ => None,
            },
        }
    }

    pub fn parse_register(
        text: &str,
        b: &mut VarBuilder,
        raw_var: bool,
    ) -> Result<(Self, Option<VarType>), String> {
        let res = parse_varstring(text, raw_var)?;
        Self::register_parsed(&res, b)
    }

    pub fn register_parsed(
        segments: &[ParsedVarStringSegment<'_>],
        builder: &mut VarBuilder,
    ) -> Result<(Self, Option<VarType>), String> {
        use ParsedVarStringSegment::*;
        let mut parts = Vec::with_capacity(segments.len());
        let mut vtypes = Vec::with_capacity(segments.len());
        for frag in segments {
            let (part, ty) = match frag {
                Text(t) => (
                    VarStringSegment::Text(t.as_bytes().to_vec()),
                    Some(VarType::Text),
                ),
                VarOrText { func, text } => {
                    let func = func.to_quoted(|name| builder.has_var(name));
                    builder
                        .register_var(func.name, func.args())?
                        .map(|(symbol_id, var_type)| (VarStringSegment::Var(symbol_id), var_type))
                        .unwrap_or_else(|| {
                            (
                                VarStringSegment::Text(text.as_bytes().to_vec()),
                                Some(VarType::Text),
                            )
                        })
                }
                Var(func) => {
                    let func = func.to_quoted(|name| builder.has_var(name));
                    builder
                        .register_var(func.name, func.args())?
                        .map(|(symbol_id, var_type)| (VarStringSegment::Var(symbol_id), var_type))
                        .ok_or_else(|| format!("Unknown variable/function: {}", func.name))?
                }
                #[cfg(feature = "expr")]
                Expr(e) => {
                    let (symbol_id, var_type) = builder.register_expr(e)?;
                    (VarStringSegment::Var(symbol_id), var_type)
                }
            };
            parts.push(part);
            vtypes.push(ty);
        }
        let vtype = if parts.len() == 1 {
            vtypes[0].take()
        } else {
            Some(VarType::Text)
        };
        Ok((Self::from_segments(&parts), vtype))
    }

    /// Splits the VarString at the first occurrence of a given separator.
    /// This is used for parsing slice ranges (start:end)
    /// The implementation is not particularly efficient, but this method is only rarely called
    pub fn split_at(&self, sep: &[u8]) -> Option<(Self, Self)> {
        for i in 0..self.len() {
            if let VarStringSegment::Text(ref t) = self.parts[i] {
                if let Some(pos) = memmem::find(t, sep) {
                    let mut start = self.parts[..i + 1].to_owned();
                    if let VarStringSegment::Text(ref mut t) = start[i] {
                        t.truncate(pos);
                    } else {
                        unreachable!();
                    }
                    let mut end = self.parts[i..].to_owned();
                    if let VarStringSegment::Text(ref mut t) = end[0] {
                        *t = t.split_off(pos + sep.len());
                    } else {
                        unreachable!();
                    }
                    return Some((Self::from_segments(&start), Self::from_segments(&end)));
                }
            }
        }
        None
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

    /// Compose the variable string given a filled symbol table
    /// Caution: the string is not cleared, any data is appended! clear it by yourself if needed
    #[inline]
    pub fn compose<W: io::Write + ?Sized>(
        &self,
        out: &mut W,
        symbols: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> io::Result<()> {
        if let Some(id) = self.one_var {
            symbols.get(id).to_text(record, out)?;
        } else {
            for part in &self.parts {
                match part {
                    VarStringSegment::Text(s) => {
                        out.write_all(s)?;
                    }
                    VarStringSegment::Var(id) => {
                        symbols.get(*id).to_text(record, out)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Obtains the integer value
    #[inline]
    pub fn get_int(
        &self,
        table: &var::symbols::SymbolTable,
        record: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) -> Result<Option<i64>, String> {
        if let Some(id) = self.one_var {
            return table.get(id).inner().map(|v| v.get_int(record)).transpose();
        }
        text_buf.clear();
        self.compose(text_buf, table, record).unwrap();
        if text_buf.as_slice() != NA.as_bytes() {
            return Ok(Some(parse_int(text_buf)?));
        }
        Ok(None)
    }

    /// Returns a SimpleValue (text/numeric/none).
    /// Requires an extra 'text_buf', which allows retaining text allocations
    /// in case values switch between the different types.
    #[inline]
    pub fn simple_value<'a>(
        &self,
        out: &'a mut SimpleValue,
        text_buf: &'a mut Vec<u8>,
        symbols: &var::symbols::SymbolTable,
        record: &dyn Record,
    ) -> Result<(), String> {
        if let Some(symbol_id) = self.one_var {
            out.replace_from_symbol(symbols.get(symbol_id), record, text_buf);
        } else {
            // try obtaining already allocated text from the value
            // like it is done in `replace_from_symbol` (see comment there)
            if let SimpleValue::Text(t) = out {
                *text_buf = mem::take(t).into_vec();
            }
            text_buf.clear();
            self.compose(text_buf, symbols, record).unwrap();
            if text_buf.as_slice() != NA.as_bytes() {
                *out = SimpleValue::Text(mem::take(text_buf).into_boxed_slice());
            } else {
                *out = SimpleValue::None;
            }
        }
        Ok(())
    }
}

impl Deref for VarString {
    type Target = [VarStringSegment];

    fn deref(&self) -> &Self::Target {
        &self.parts
    }
}
