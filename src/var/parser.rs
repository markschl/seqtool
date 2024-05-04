//! Simple JavaScript source parser for recognizing variables/functions
//! and replacing them with placeholder variables.
//! It *should* not fail with any valid JavaScript (otherwise there is a bug),
//! except for code containing literal /regexp/ notation, which is not supported,
//! since it is difficult to correctly parse without any knowledge
//! of the context.
//!
//! The module also offers functions for parsing 'seqtool'-like variables/functions
//! (same as JS parser) and 'variable strings'.
//! Therefore, some functions of this module are still used without the "expr" feature.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use var_provider::FromArg;
use winnow::ascii::{multispace0, space0};
use winnow::combinator::{
    alt, cond, delimited, eof, not, opt, peek, preceded, repeat, separated, terminated,
};
use winnow::stream::{AsChar, Compare, FindSlice, Location, Offset, Stream, StreamIsPartial};
use winnow::token::{any, one_of, take_till, take_while};
use winnow::{Located, PResult, Parser};

use crate::CliError;

use super::modules::expr::js::parser::{expression, Expression};

/// Function or simple variable, which was found to match the seqtool-style
/// variable/function syntax by the parser.
/// This is the type returned by the parser, all data is borrowed from the
/// underlying variable/function/expression if possible.
#[derive(Debug, Clone, PartialEq)]
pub struct VarFunc<'a> {
    pub name: &'a str,
    pub args: Option<Vec<Arg<'a>>>,
    pub range: Range<usize>,
}

impl<'a> VarFunc<'a> {
    pub fn new(name: &'a str, args: Option<Vec<Arg<'a>>>, range: Range<usize>) -> Self {
        Self { name, args, range }
    }

    pub fn args(&'a self) -> &'a [Arg<'a>] {
        self.args.as_deref().unwrap_or_default()
    }

    /// Attempts to convert the `VarFunc` to a 'seqtool'-style function, which
    /// in contrast to JavaScript only allows for certain argument types.
    ///
    /// Unquoted strings are also allowed in seqtool-style functions in certain contexts,
    /// which makes everything a bit complicated, unquoted strings could also in fact be
    /// plain seqtool variables provided as function arguments.
    /// This is the reason why we need a lookup function checking if a string is a
    /// known seqtool variable.
    /// If the user supplies a known seqtool variable name, but actually means it
    /// to be an unquoted string, there will be a type error warning about this
    /// problem.
    ///
    /// Current rules:
    /// (1) the text is not identical with a known variable or function,
    /// (2) no more complicated expressions such as {attr(x) + 1}."
    pub fn to_quoted<F>(&self, mut lookup_fn: F) -> VarFunc<'a>
    where
        F: FnMut(&str) -> bool,
    {
        self._to_quoted(&mut lookup_fn)
    }

    pub fn _to_quoted(&self, lookup_fn: &mut impl FnMut(&str) -> bool) -> VarFunc<'a> {
        if self.args.is_none() {
            return self.clone();
        }
        let mut out = self.clone();
        let args = out.args.as_mut().unwrap();
        for arg in args {
            if let Arg::Func(func) = arg {
                // The unquoted string must *not* be a valid variable and
                // variables ending with '()' (= function call)
                // cannot be interpreted as an unquoted string
                if func.args.is_none() {
                    if !lookup_fn(func.name) {
                        *arg = Arg::Str(func.name.into());
                    }
                } else {
                    // we also try quoting nested args
                    *arg = Arg::Func(func._to_quoted(lookup_fn));
                }
            }
        }
        out
    }
}

impl fmt::Display for VarFunc<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(args) = self.args.as_ref() {
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                a.fmt(f)?;
            }
        }
        Ok(())
    }
}

/// Argument type for functions that match the seqtool-style function syntax.
/// This is the type returned by the parser, all data is borrowed from the
/// underlying variable/function/expression if possible.
///
/// (but see `Expr` variant, which is only used internally and cannot be
/// constructed from a parsed function)
#[derive(Debug, Clone, PartialEq)]
pub enum Arg<'a> {
    /// All kinds of standard string or numeric/boolan arguments, which may be further
    /// converted to their respective types by the responsible variable providers.
    Str(Cow<'a, str>),
    /// Seqtool-style functions can be supplied as arguments to certain
    /// other functions (e.g. `Num(...)`).
    // /// - identifiers (with no arguments) may actually be interpreted as
    // ///   unquoted strings
    Func(VarFunc<'a>),
    /// Parsed JS expressions can be supplied as arguments to specialized
    /// "functions", which are only used internally: _____expr(paresed_expression)
    /// is used to shuttle the already parsed expression to the variable provider
    /// responsible for the expression evaluation.
    #[cfg(feature = "expr")]
    Expr(Expression<'a>),
}

impl fmt::Display for Arg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::Str(s) => s.fmt(f),
            Arg::Func(func) => func.fmt(f),
            #[cfg(feature = "expr")]
            Arg::Expr(expr) => expr.fmt(f),
        }
    }
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Self::Str(s.to_string().into())
    }
}

macro_rules! impl_from_arg {
    ($ty:ty, $what:expr) => {
        impl<'a> FromArg<&'a Arg<'a>> for $ty {
            fn from_arg(
                func_name: &str,
                arg_name: &str,
                value: &'a Arg<'a>,
            ) -> Result<Self, String> {
                match value {
                    Arg::Str(s) => <$ty>::from_arg(func_name, arg_name, s.as_ref()),
                    Arg::Func(f) => Err(format!(
                        "Invalid value supplied to the function '{}': '{}' (argument '{}'). Expected a {}, but found a {}.{}",
                        func_name,
                        value,
                        arg_name,
                        $what,
                        if f.args.is_some() { "function" } else { "variable" },
                        if f.args.is_some() { "".to_string() } else { format!(
                            " In case it was intended to be a string, try enclosing in single \
                            or double quotes instead ('{}' or \"{}\"). \
                            Unquoted strings are allowed in simple cases, but not here.", value, value
                        )})),
                    _ => unreachable!(), // should only ever occur internally
                }
            }
        }
    };
}

impl_from_arg!(usize, "number");
impl_from_arg!(f64, "number");
impl_from_arg!(bool, "boolean (true/false)");
impl_from_arg!(String, "string");

impl<'a> FromArg<&'a Arg<'a>> for Arg<'a> {
    fn from_arg(_: &str, _: &str, value: &'a Arg<'a>) -> Result<Self, String> {
        Ok(value.clone())
    }
}

impl<'a> FromArg<&'a Arg<'a>> for VarFunc<'a> {
    fn from_arg(func_name: &str, _: &str, value: &'a Arg<'a>) -> Result<Self, String> {
        match value {
            Arg::Func(f) => Ok(f.clone()),
            _ => Err(format!(
                "Cannot convert the argument '{}' of '{}' a function",
                func_name, value
            )),
        }
    }
}

#[cfg(feature = "expr")]
impl<'a> FromArg<&'a Arg<'a>> for Expression<'a> {
    fn from_arg(_: &str, _: &str, value: &'a Arg<'a>) -> Result<Self, String> {
        match value {
            // TODO: clone necessary
            Arg::Expr(e) => Ok(e.clone()),
            _ => unreachable!(), // should only ever occur internally
        }
    }
}

/// Variable string parts, together forming a whole variable string if ordered in a sequence.
///
/// The function/expression syntax may still be invalid. Variable/function registration
/// will only be done when constructing a `VarString` with `VarString::from_parsed()`.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedVarStringSegment<'a> {
    /// seqtool-style variable/function
    Var(VarFunc<'a>),
    /// either a function or string (validity of function will be checked later)
    VarOrText {
        func: VarFunc<'a>,
        text: &'a str,
    },
    Text(&'a str),
    #[cfg(feature = "expr")]
    Expr(Expression<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarStringParseErr(pub(crate) String);

impl VarStringParseErr {
    #[cold]
    pub fn new(rest: &str) -> Self {
        let mut rest = rest.to_string();
        if rest.len() > 100 {
            rest.truncate(100);
            rest.push_str("...");
        }
        Self(rest)
    }
}

#[cfg(not(feature = "expr"))]
// with the 'expr' feature, we use a different message
impl std::fmt::Display for VarStringParseErr {
    #[cold]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Failed to parse the following string with variables/functions: \
            \n`{}`\n\
            Make sure to correctly close all parentheses and {{ brackets }} \
            indicating a variable/function. \
            Examples: {{ num }} or {{ attr('a') }}.",
            self.0
        )
    }
}

impl From<VarStringParseErr> for CliError {
    fn from(err: VarStringParseErr) -> CliError {
        CliError::Other(err.to_string())
    }
}

pub trait StrStream<'a>:
    Stream<Slice = &'a str, Token = char>
    + StreamIsPartial
    + Compare<&'a str>
    + Compare<char>
    + FindSlice<&'a str>
    + FindSlice<char>
    + Clone
    + Offset
{
}

pub trait LocatedStream<'a>: StrStream<'a> + Location {}

impl<'a> StrStream<'a> for &'a str {}
impl<'a, S: StrStream<'a>> StrStream<'a> for Located<S> {}
impl<'a, S: StrStream<'a>> LocatedStream<'a> for Located<S> {}

pub fn parse_varstring(text: &str, raw_var: bool) -> Result<Vec<ParsedVarStringSegment>, String> {
    let mut input = Located::new(text);
    let frags = varstring(raw_var, None).parse_next(&mut input).unwrap();
    if input.len() > 0 {
        return Err(VarStringParseErr::new(&input).to_string());
    }
    // dbg!("varstring", &frags);
    Ok(frags)
}

pub fn parse_varstring_list(
    text: &str,
    raw_var: bool,
) -> Result<Vec<Vec<ParsedVarStringSegment>>, String> {
    let mut input = Located::new(text);
    let frags = varstring_list(raw_var).parse_next(&mut input).unwrap();
    if input.len() > 0 {
        return Err(VarStringParseErr::new(&input).to_string());
    }
    // dbg!("varstring list", &frags);
    Ok(frags)
}

fn varstring_list<'a, S: LocatedStream<'a>>(
    raw_var: bool,
) -> impl FnMut(&mut S) -> PResult<Vec<Vec<ParsedVarStringSegment<'a>>>> {
    move |input: &mut S| separated(1.., varstring(raw_var, Some(',')), ',').parse_next(input)
}

fn varstring<'a, S: LocatedStream<'a>>(
    raw_var: bool,
    stop_at: Option<char>,
) -> impl FnMut(&mut S) -> PResult<Vec<ParsedVarStringSegment<'a>>> {
    move |input: &mut S| {
        if !raw_var {
            _varstring(stop_at).parse_next(input)
        } else {
            let res = alt((
                terminated(
                    delimited(multispace0, var_or_func, multispace0)
                        .with_recognized()
                        .map(|(func, text)| vec![ParsedVarStringSegment::VarOrText { func, text }]),
                    // ensure that the next char is either a separator (stop_at) or EOF
                    peek(alt((
                        // TODO: quite complicated, '\0' never used
                        //       and verify() makes sure that the parser fails if stop_at is None
                        cond(stop_at.is_some(), stop_at.unwrap_or('\0'))
                            .verify(|o| o.is_some())
                            .recognize(),
                        eof,
                    ))),
                ),
                _varstring(stop_at),
            ))
            .parse_next(input)?;
            Ok(res)
        }
    }
}

fn _varstring<'a, S: LocatedStream<'a>>(
    stop_at: Option<char>,
) -> impl FnMut(&mut S) -> PResult<Vec<ParsedVarStringSegment<'a>>> {
    move |input: &mut S| repeat(.., varstring_fragment(stop_at)).parse_next(input)
}

/// Variable/function string parser
fn varstring_fragment<'a, S: LocatedStream<'a>>(
    stop_at: Option<char>,
) -> impl FnMut(&mut S) -> PResult<ParsedVarStringSegment<'a>> {
    move |input: &mut S| {
        alt((
            // { var } or { func(a, b) }
            delimited(
                ("{", space0),
                var_or_func.map(ParsedVarStringSegment::Var),
                (space0, "}"),
            ),
            // {file:path.js} or { expression }
            // #[cfg(feature = "expr")]
            delimited("{", expression, "}").map(ParsedVarStringSegment::Expr),
            // text
            // TODO: escaping '{' not possible
            take_till(1.., |c| {
                c == '{' || stop_at.map(|s| c == s).unwrap_or(false)
            })
            .map(ParsedVarStringSegment::Text),
        ))
        .parse_next(input)
    }
}

// /// Succeeds only if variable (generally identifier) or function-like syntax encountered
// #[inline(never)]
// fn var_or_expr<'a, S: StrStream<'a>>(input: &mut S) -> PResult<Function<'a>> {
//     alt((var_or_func, expression))
// }

/// Succeeds only if variable (generally identifier) or function-like syntax encountered
#[inline(never)]
pub fn var_or_func<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<VarFunc<'a>> {
    (
        name,
        alt((
            not((multispace0, "(")).recognize().map(|_| None),
            delimited('(', arg_list, ')').map(Some),
        )),
    )
        .with_span()
        .map(|((name, args), rng)| VarFunc::new(name, args, rng))
        .parse_next(input)
}

/// Matches valid variable/function names, not only for internal 'seqtool-style' names, but also
/// compatible with JS identifiers, except for \u escape sequences
/// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#identifiers
#[inline(never)]
pub(crate) fn name<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    (
        take_while(1, |c: char| c.is_alphabetic() || c == '_' || c == '$'), // must not start with number
        take_while(0.., |c: char| c.is_alphanumeric() || c == '_' || c == '$'),
    )
        .recognize()
        .parse_next(input)
}

// parens in math expressions, function arguments, etc.
fn arg_list<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Arg<'a>>> {
    terminated(
        separated(.., delimited(multispace0, arg, multispace0), ','),
        opt((',', multispace0)),
    )
    .parse_next(input)
}

// parens in math expressions, function arguments, etc.
fn arg<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Arg<'a>> {
    alt((
        var_or_func.map(Arg::Func),
        string.map(Arg::Str),
        some_value.map(|v| Arg::Str(v.into())),
    ))
    .parse_next(input)
}

/// Numeric values, booleans or special non-quoted strings (allowed by seqtool to some extent).
/// To be precise: values that don't qualify as "names", e.g. start
/// with number or contain '-' or '.'.
/// TODO: allow more special characters in unquoted strings?
pub(crate) fn some_value<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanum() || c == '_' || c == '-' || c == '.'
    })
    .parse_next(input)
}

// 'string' or "string" (quote escapes possible)
pub(crate) fn string<'a, S: StrStream<'a>>(input: &mut S) -> PResult<Cow<'a, str>> {
    alt((quoted('"'), quoted('\''))).parse_next(input)
}

fn quoted<'a, S: StrStream<'a>>(quote: char) -> impl FnMut(&mut S) -> PResult<Cow<'a, str>> {
    move |input: &mut S| {
        delimited(
            quote,
            repeat(.., string_fragment(&[quote])).fold(
                || Cow::Borrowed(""),
                |mut out, fragment| {
                    if let Cow::Borrowed(s) = &mut out {
                        if s.is_empty() {
                            *s = fragment;
                        } else {
                            out = Cow::Owned(fragment.to_string());
                        }
                    }
                    if let Cow::Owned(s) = &mut out {
                        s.push_str(fragment);
                    }
                    out
                },
            ),
            quote,
        )
        .parse_next(input)
    }
}

pub fn string_fragment<'a, S: StrStream<'a>>(
    stop: &[char],
) -> impl FnMut(&mut S) -> PResult<&'a str> + '_ {
    move |input: &mut S| {
        alt((
            // regular non-escaped string
            take_till(1.., (stop, '\\')),
            // ignore escaped quotes
            alt((
                // remove backslashes in case of escaped quotes or double escapes
                preceded('\\', alt((one_of(stop).recognize(), "\\"))),
                // any other type of character escape: just leave as-is
                // important: the string should not end with an escape char, so we have to consume the next character
                ('\\', any).recognize(),
            )),
        ))
        .parse_next(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varstring_list() {
        use ParsedVarStringSegment::*;
        let input = "{func('a', b, 1.24)}_rest , id , {id}, a(";
        let res = parse_varstring_list(input, false);
        let exp = vec![
            vec![
                Var(VarFunc::new(
                    "func",
                    Some(vec![
                        "a".into(),
                        Arg::Func(VarFunc::new("b", None, 11..12)),
                        "1.24".into(),
                    ]),
                    1..19,
                )),
                Text("_rest "),
            ],
            vec![Text(" id ")],
            vec![Text(" "), Var(VarFunc::new("id", None, 34..36))],
            vec![Text(" a(")],
        ];
        assert_eq!(res.unwrap(), exp);
        // allowing parts without { braces }
        let res = parse_varstring_list(input, true);
        let exp = vec![
            vec![
                Var(VarFunc::new(
                    "func",
                    Some(vec![
                        "a".into(),
                        Arg::Func(VarFunc::new("b", None, 11..12)),
                        "1.24".into(),
                    ]),
                    1..19,
                )),
                Text("_rest "),
            ],
            vec![VarOrText {
                func: VarFunc::new("id", None, 28..30),
                text: " id ",
            }],
            vec![Text(" "), Var(VarFunc::new("id", None, 34..36))],
            vec![Text(" a(")],
        ];
        assert_eq!(res.unwrap(), exp);
    }
}
