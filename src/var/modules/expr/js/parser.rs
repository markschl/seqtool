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

use std::ops::Range;

use winnow::ascii::{multispace0, multispace1, space0};
use winnow::combinator::{
    alt, cond, delimited, eof, opt, peek, preceded, repeat, separated, terminated,
};
use winnow::stream::{AsChar, Compare, FindSlice, Stream, StreamIsPartial};
use winnow::token::{any, one_of, take_till, take_until, take_while};
use winnow::{Located, PResult, Parser};

use crate::error::{CliError, CliResult};
use crate::var::func::Func;

/// Variable string parts, which form a whole variable string if ordered in a sequence.
/// The `F` parameter indicates the 'function' type. Either a `Fragment` (obtained from a first parsing step,
/// where the focus is only on valid syntax), or a `Func` type (obtained in a second step, further validating
/// the function name/args).
/// The function/expression syntax may still be invalid. Variable/function registration will only be done
/// when constructing a `VarString` with `VarString::from_parsed()`.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedVarStringSegment<'a, F> {
    #[cfg(feature = "expr")]
    Expr(&'a str),
    #[cfg(feature = "expr")]
    SourceFile(&'a str),
    Var(F),
    VarOrText(F, &'a str),
    Text(&'a str),
}

impl<'a> TryFrom<ParsedVarStringSegment<'a, Fragment<'a>>> for ParsedVarStringSegment<'a, Func> {
    type Error = String;

    fn try_from(segment: ParsedVarStringSegment<'a, Fragment<'a>>) -> Result<Self, String> {
        Ok(match segment {
            #[cfg(feature = "expr")]
            ParsedVarStringSegment::Expr(s) => ParsedVarStringSegment::Expr(s),
            #[cfg(feature = "expr")]
            ParsedVarStringSegment::SourceFile(s) => ParsedVarStringSegment::SourceFile(s),
            ParsedVarStringSegment::Var(f) => {
                // var_or_func already made sure that the fragment is a var/function
                let (name, args, _rng) = f.get_func().unwrap();
                ParsedVarStringSegment::Var(st_func_from_parsed(name, args, true)?)
            }
            ParsedVarStringSegment::VarOrText(f, t) => {
                let (name, args, _rng) = f.get_func().unwrap();
                if let Ok(f) = st_func_from_parsed(name, args, true) {
                    ParsedVarStringSegment::VarOrText(f, t)
                } else {
                    ParsedVarStringSegment::Text(t)
                }
            }
            ParsedVarStringSegment::Text(s) => ParsedVarStringSegment::Text(s),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarStringParseErr(String);

impl VarStringParseErr {
    #[inline(never)]
    pub fn new(rest: &str) -> Self {
        let mut rest = rest.to_string();
        if rest.len() > 100 {
            rest.truncate(100);
            rest.push_str("...");
        }
        Self(rest)
    }
}

impl std::fmt::Display for VarStringParseErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,
            "Failed to parse the following string with variables/functions or JavaScript code: \
            \n`{}`\n\
            Make sure that variables/functions are in the form {{variable}} or {{function(arg)}} \
            and expressions/scripts in the form {{{{ <JavaScript...> }}}} or {{{{ file:path/to/script.js }}}}. \
            Ensure that every parenthesis '{{' is closed with '}}'. \
            Check for syntax errors in JavaScript expressions \
            and don't use regular expressions with the /regex/ notation \
            (unsupported; use `new RegExp(\"regex\")` instead). \
            General help: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Language_overview \
            ",
            self.0
        )
    }
}

impl From<VarStringParseErr> for CliError {
    fn from(e: VarStringParseErr) -> Self {
        CliError::Other(e.to_string())
    }
}

pub fn parse_varstring(text: &str, raw_var: bool) -> CliResult<Vec<ParsedVarStringSegment<Func>>> {
    let mut input = Located::new(text);
    let frags = varstring(raw_var, None).parse_next(&mut input).unwrap();
    if input.len() > 0 {
        return Err(VarStringParseErr::new(&input).into());
    }
    let frags = frags
        .into_iter()
        .map(|f| f.try_into())
        .collect::<Result<_, _>>()?;
    Ok(frags)
}

pub fn parse_varstring_list(
    text: &str,
    raw_var: bool,
) -> CliResult<Vec<Vec<ParsedVarStringSegment<Func>>>> {
    let mut input = Located::new(text);
    let frags = varstring_list(raw_var).parse_next(&mut input).unwrap();
    if input.len() > 0 {
        return Err(VarStringParseErr::new(&input).into());
    }
    let frags = frags
        .into_iter()
        .map(|f| {
            f.into_iter()
                .map(|f| f.try_into())
                .collect::<Result<_, _>>()
        })
        .collect::<Result<_, _>>()?;
    Ok(frags)
}

fn varstring<'a>(
    raw_var: bool,
    stop_at: Option<char>,
) -> impl FnMut(&mut Located<&'a str>) -> PResult<Vec<ParsedVarStringSegment<'a, Fragment<'a>>>> {
    move |input: &mut Located<&'a str>| {
        if !raw_var {
            _varstring(stop_at).parse_next(input)
        } else {
            let res = alt((
                terminated(
                    delimited(multispace0, var_or_func, multispace0)
                        .with_recognized()
                        .map(|(f, r)| vec![ParsedVarStringSegment::VarOrText(f, r)]),
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

fn varstring_list<'a>(
    raw_var: bool,
) -> impl FnMut(&mut Located<&'a str>) -> PResult<Vec<Vec<ParsedVarStringSegment<'a, Fragment<'a>>>>>
{
    move |input: &mut Located<&'a str>| {
        separated(1.., varstring(raw_var, Some(',')), ',').parse_next(input)
    }
}

fn _varstring<'a>(
    stop_at: Option<char>,
) -> impl FnMut(&mut Located<&'a str>) -> PResult<Vec<ParsedVarStringSegment<'a, Fragment<'a>>>> {
    move |input: &mut Located<&'a str>| repeat(.., varstring_fragment(stop_at)).parse_next(input)
}

/// Succeeds only if variable (generally identifier) or function-like syntax encountered
#[inline(never)]
fn var_or_func<'a>(input: &mut Located<&'a str>) -> PResult<Fragment<'a>> {
    item.verify_map(|opt_frag| {
        opt_frag.and_then(|f| match f {
            Fragment::Func(..) | Fragment::Name(..) => Some(f),
            _ => None,
        })
    })
    .parse_next(input)
}

/// Variable/function string parser
fn varstring_fragment<'a>(
    stop_at: Option<char>,
) -> impl FnMut(&mut Located<&'a str>) -> PResult<ParsedVarStringSegment<'a, Fragment<'a>>> {
    move |input: &mut Located<&'a str>| {
        alt((
            // {{ file:path.js }}
            #[cfg(feature = "expr")]
            delimited(
                ("{{", multispace0),
                preceded("file:", take_until(1.., "}}")),
                "}}",
            )
            .map(ParsedVarStringSegment::SourceFile),
            // {{ expr }}
            #[cfg(feature = "expr")]
            delimited("{{", statements.recognize(), "}}").map(ParsedVarStringSegment::Expr),
            // { var } or { func(a, b) }
            delimited("{", delimited(space0, var_or_func, space0), "}")
                .map(ParsedVarStringSegment::Var),
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

/// Simplified "AST" representation of a script, which is tailored
/// towards approximately (but correctly) parsing the JavaScript syntax in order
/// to be able to recognize internal seqtool variables/functions, which
/// are used in the script.
///
/// The tree does not represent any relationships formed by operators,
/// definitions and assignments. Statements within the same block are
/// represented as as a sequence of elements (Fragment).
/// The parser currently cannot recognize:
/// - JavaScript RegExp literals
///   (difficult or impossible? to parse without deeper knowledge of the context),
/// - Unicode escape sequences \u... or \u{...} in variable/function names
/// - Certain exotic Unicode characters that are not char::is_alphabetic()
///
/// The parser also cannot distinghish between function calls and function definitions,
/// variable usage and variable assignments, etc. So, there should not be any
/// variable or function definitions with the same name as seqtool variables/functions,
/// this will lead to confusion.
/// TODO: maybe try to recognize cases such as 'var id...' or 'function meta(...)' in the future and warn about those
#[derive(Debug, Clone)]
pub struct PseudoAst<'a> {
    script: &'a str,
    frags: Vec<Fragment<'a>>,
}

impl<'a> PseudoAst<'a> {
    /// Extracts functions/variables from the script, calling `reg_fn` on every of those.
    /// `reg_fn` should return the name of the placeholder variable, which is then
    /// defined by 'seqtool' (functions are all replaced by static variables).
    /// Returns the rewritten script.
    /// `reg_fn` may return `None` if the variable/function is unknown
    /// (meaning that it is likely a JavaScript function) or Some(Err("..."))
    /// if the arguments are invalid.
    pub fn rewrite<F, E>(&self, mut reg_fn: F) -> Result<String, E>
    where
        F: FnMut(&str, &[Fragment]) -> Option<Result<String, E>>,
        E: From<String>,
    {
        let mut vars = Vec::new();
        _obtain_vars(&self.frags, &mut vars, 0)?;
        // sort by order of occurrence
        vars.sort_by_key(|v| v.0.start);
        // rewrite the script sequentially
        let mut out = String::with_capacity(self.script.len());
        let mut prev_end = 0;
        for (rng, name, args) in vars {
            if let Some(res) = reg_fn(name, args) {
                let replacement = res?;
                debug_assert!(rng.start >= prev_end);
                out.push_str(&self.script[prev_end..rng.start]);
                out.push_str(&replacement);
                prev_end = rng.end;
            }
        }
        out.push_str(&self.script[prev_end..]);
        Ok(out)
    }
}

fn _obtain_vars<'a>(
    node: &'a [Fragment<'a>],
    out: &mut Vec<(Range<usize>, &'a str, &'a [Fragment<'a>])>,
    offset: usize,
) -> Result<(), String> {
    for frag in node {
        if let Some((name, args, rng)) = frag.get_func() {
            let rng = Range {
                start: rng.start + offset,
                end: rng.end + offset,
            };
            out.push((rng, name, args));
        }
        if let Fragment::Func(_, nested, _) | Fragment::Nested(nested) = frag {
            _obtain_vars(nested, out, offset)?;
        }
        // Fragment::Regex(r, rng) => {
        //     let mut loc = Located::new(*r);
        //     if let Ok(frags) = _parse_script(&mut loc) {
        //         if loc.is_empty() {
        //             _get_vars(&frags, out, rng.start);
        //         }
        //     }
        // }
    }
    Ok(())
}

/// Parses a script into a simplified AST, optimized for small binary size rather
/// than speed or complex syntax recognition.
/// Does *not* recognize /regex notation/, since it is difficult to parse.
/// Returns Err(rest) if only part of the script was parsed.
pub fn parse_script(script: &str) -> Result<PseudoAst<'_>, String> {
    // parse the script
    let mut input = Located::new(script);
    let frags = _parse_script(&mut input).unwrap();
    if input.len() > 0 {
        return Err(input.to_string());
    }
    // dbg!(&frags);
    Ok(PseudoAst { script, frags })
}

fn _parse_script<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    statements.parse_next(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fragment<'a> {
    // variable/function/class names
    Name(&'a str, Range<usize>),
    // any kind of operator
    Operator(char),
    // function calls or definitions, object construction
    Func(&'a str, Vec<Fragment<'a>>, Range<usize>),
    // reserved/builtin keyword
    Builtin(&'a str),
    // numeric literal,
    Value(&'a str),
    // string literal or part of a string
    String(&'a str),
    // list of statements in a block scope, list of function arguments,
    // array elements, etc.
    Nested(Vec<Fragment<'a>>),
    // Regex(&'a str, Range<usize>),
}

impl<'a> Fragment<'a> {
    fn get_func(&'a self) -> Option<(&'a str, &'a [Fragment<'a>], Range<usize>)> {
        match self {
            Fragment::Name(name, rng) => Some((name, [].as_slice(), rng.clone())),
            Fragment::Func(name, args, rng) => {
                if *name == "." {
                    // '.' stands for member access (see member_access)
                    return None;
                }
                Some((name, args.as_slice(), rng.clone()))
            }
            _ => None,
        }
    }
}

pub fn st_func_from_parsed(
    name: &str,
    args: &[Fragment<'_>],
    allow_unquoted: bool,
) -> Result<Func, String> {
    let str_args = args
        .iter()
        .map(|v| match v {
            Fragment::Name(_, _) if !allow_unquoted => {
                Err("Unquoted arguments are not allowed in this context.")
            }
            Fragment::String(s) | Fragment::Name(s, _) | Fragment::Value(s) => Ok(s.to_string()),
            _ => Err("Seqtool function arguments must be strings or numbers."),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Invalid function: '{}'. {}", name, e))?;
    Ok(Func::new(name, &str_args))
}

trait StrStream<'a>:
    Stream<Slice = &'a str, Token = char>
    + StreamIsPartial
    + Compare<&'a str>
    + FindSlice<&'a str>
    + FindSlice<char>
{
}

impl<'a> StrStream<'a> for &'a str {}
impl<'a> StrStream<'a> for Located<&'a str> {}

/// "statement" (sequence of 'item' terminated by semicolon or comma;
/// even though these are not generally interchangeable
fn stmt<'a>(input: &mut Located<&'a str>) -> PResult<Option<Fragment<'a>>> {
    item_terminated(&[';', ',']).parse_next(input)
}

/// List of "statements" (without ignored parts)
#[inline(never)]
fn statements<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    repeat(.., stmt)
        .fold(Vec::new, |mut acc: Vec<_>, frag| {
            if let Some(f) = frag {
                acc.push(f)
            }
            acc
        })
        .parse_next(input)
}

fn item_terminated<'a>(
    delim: &[char],
) -> impl FnMut(&mut Located<&'a str>) -> PResult<Option<Fragment<'a>>> + '_ {
    move |input: &mut Located<&'a str>| {
        let o = input.eof_offset();
        let e = opt_item.parse_next(input)?;
        if input.eof_offset() == o {
            // make sure output is not empty
            one_of(delim).void().parse_next(input)?;
        } else {
            opt(one_of(delim)).void().parse_next(input)?;
        }
        Ok(e)
    }
}

fn opt_item<'a>(input: &mut Located<&'a str>) -> PResult<Option<Fragment<'a>>> {
    alt((item, multispace0.map(|_| None))).parse_next(input)
}

/// Javascript "items" (keywords, variables, operators, blocks, function calls, arrays, etc)
#[inline(never)]
fn item<'a>(input: &mut Located<&'a str>) -> PResult<Option<Fragment<'a>>> {
    alt((
        alt((multispace1, comment, inline_comment)).map(|_| None),
        alt((
            block.map(Fragment::Nested),
            index_list.map(Fragment::Nested),
            parens_list.map(Fragment::Nested),
            operator.map(Fragment::Operator),
            string.map(Fragment::String),
            template_string.map(Fragment::Nested),
            func.with_span()
                .map(|((name, args), s)| Fragment::Func(name, args, s)),
            name.with_span().map(|(v, s)| {
                if BUILTINS.iter().any(|b| b == &v) {
                    Fragment::Builtin(v)
                } else {
                    Fragment::Name(v, s)
                }
            }),
            // regex.with_span().map(|(r, s)| Fragment::Regex(r, s)),
            member_access
                .with_span()
                .map(|(m, rng)| Fragment::Func(".", vec![m], rng)),
            some_value.map(Fragment::Value),
        ))
        .map(Some),
    ))
    .parse_next(input)
}

// block scopes (function body, class definition, etc.), object notation
fn block<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    delimited('{', statements, '}').parse_next(input)
}

// arrays: [a, b, c], array indexing, etc.
// For simplicity, we allow semicolons as well, even though they are invalid in JS
fn index_list<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    delimited('[', statements, ']').parse_next(input)
}

// parens in math expressions, function arguments, etc.
fn parens_list<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    delimited('(', statements, ')').parse_next(input)
}

// .member or .member_fn()
fn member_access<'a>(input: &mut Located<&'a str>) -> PResult<Fragment<'a>> {
    (
        ".",
        alt((
            func.with_span()
                .map(|((name, args), s)| Fragment::Func(name, args, s)),
            name.with_span().map(|(v, s)| Fragment::Name(v, s)),
        )),
    )
        .map(|(_, member)| member)
        .parse_next(input)
}

// func_name(a, b) or if/for/while, etc.
#[inline(never)]
fn func<'a>(input: &mut Located<&'a str>) -> PResult<(&'a str, Vec<Fragment<'a>>)> {
    (name, parens_list).parse_next(input)
}

/// Matches valid variable names, as well as function/class names,
/// except for \u escape sequences
/// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Lexical_grammar#identifiers
#[inline(never)]
fn name<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    (
        take_while(1, |c: char| c.is_alphabetic() || c == '_' || c == '$'), // must not start with number
        take_while(0.., |c: char| c.is_alphanumeric() || c == '_' || c == '$'),
    )
        .recognize()
        .parse_next(input)
}

static BUILTINS: &[&str] = &[
    "null",
    "undefined",
    "true",
    "false",
    "delete",
    "debugger",
    "import",
    "export",
    "as",
    "from",
    "with",
    "for",
    "while",
    "do",
    "break",
    "continue",
    "in",
    "of",
    "try",
    "catch",
    "finally",
    "throw",
    "switch",
    "case",
    "default",
    "if",
    "else",
    "function",
    "=>",
    "void",
    "class",
    "new",
    "constructor",
    "this",
    "super",
    "return",
    "typeof",
    "instanceof",
    "static",
    "yield",
    "async",
    "await",
    "enum",
    "arguments",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "var",
    "let",
    "const",
];

/// Operators and operator-like characters (including => arrow function definition).
/// We only match single characters, thus operators such as '!=' and '>>>' will just
/// result in a sequence of single operators
fn operator<'a, S: StrStream<'a>>(input: &mut S) -> PResult<char> {
    one_of([
        '=', '!', '?', ':', '&', '|', '<', '>', '+', '-', '*', '/', '%', '^', '~',
    ])
    .parse_next(input)
}

/// Numeric values or non-quoted strings (allowed by seqtool to some extent).
/// To be precise: values that don't qualify as "names", e.g. start
/// with number or contain '-' or '.'.
fn some_value<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    take_while(1.., |c: char| {
        c.is_alphanum() || c == '_' || c == '-' || c == '.'
    })
    .parse_next(input)
}

// // comment
fn comment<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    ("//", take_till(1.., ['\n', '\r']))
        .map(|(_, s)| s)
        .parse_next(input)
}

// /* inline / multi-line comments */
fn inline_comment<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    ("/*", take_until(.., "*/"), "*/")
        .map(|(_, s, _)| s)
        .parse_next(input)
}

// 'string' or "string" (with quote escapes)
fn string<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
    alt((quoted('"'), quoted('\''))).parse_next(input)
}

fn quoted<'a, S: StrStream<'a>>(quote: char) -> impl FnMut(&mut S) -> PResult<&'a str> {
    move |input: &mut S| {
        delimited(
            quote,
            repeat::<_, _, (), _, _>(.., string_fragment(&[quote])).recognize(),
            quote,
        )
        .parse_next(input)
    }
}

fn string_fragment<'a, S: StrStream<'a>>(
    stop: &[char],
) -> impl FnMut(&mut S) -> PResult<&'a str> + '_ {
    move |input: &mut S| {
        alt((
            // regular non-escaped string
            take_till(1.., (stop, '\\')).void(),
            // any type of character escape
            // most important: the string should not end with an escape char, so we have to consume the next character
            ('\\', any).void(),
        ))
        .recognize()
        .parse_next(input)
    }
}

// `template string with ${vars}, or \${escaped}`
// note: these are *not* valid as seqtool string arguments, and may contain
// of several 'nested' parts
fn template_string<'a>(input: &mut Located<&'a str>) -> PResult<Vec<Fragment<'a>>> {
    delimited('`', repeat(.., template_fragment), '`').parse_next(input)
}

fn template_fragment<'a>(input: &mut Located<&'a str>) -> PResult<Fragment<'a>> {
    alt((
        preceded('$', block).map(Fragment::Nested),
        "$".map(Fragment::String),
        string_fragment(&['`', '$']).map(Fragment::String),
    ))
    .parse_next(input)
}

// /// /regex/: not supported since not easy to distinguish from division and comments
// /// without knowing the context
// fn regex<'a, S: StrStream<'a>>(input: &mut S) -> PResult<&'a str> {
//     delimited(
//         '/',
//         repeat::<_, _, (), _, _>(.., |i: &mut S| string_fragment(i, &['/', '\n', '\r'])).recognize(),
//         '/',
//     )
//     .parse_next(input)
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rewrite(script: &str, expected: Result<&str, &str>) {
        let res = parse_script(script).and_then(|ast| {
            ast.rewrite(|name, args| {
                let rep = match name {
                    "variable" => "var_repl".to_string(),
                    "func" => "func_repl".to_string(),
                    _ => return None,
                };
                try_opt!(st_func_from_parsed(name, args, true));
                Some(Ok::<_, String>(rep))
            })
        });
        match res {
            Ok(out) => {
                assert!(expected.is_ok());
                assert_eq!(out, expected.unwrap());
            }
            Err(e) => {
                assert!(expected.is_err());
                assert_eq!(expected.unwrap_err(), e);
            }
        }
    }

    #[test]
    fn simple() {
        test_rewrite(
            "var a = variable; var b = func('arg1', 'arg2') ? variable : 3;",
            Ok("var a = var_repl; var b = func_repl ? var_repl : 3;"),
        );
    }

    #[test]
    fn nested() {
        test_rewrite(
            "const a = ['quoted variable', \"item2\", 'qu\\'oted', func('a')];
            { someobject.method(a[0]).method(variable, func('arg1', 'arg2')); }",
            Ok(
                "const a = ['quoted variable', \"item2\", 'qu\\'oted', func_repl];
            { someobject.method(a[0]).method(var_repl, func_repl); }",
            ),
        );
        test_rewrite(
            "variable + func('a') + func() / variable;",
            Ok("var_repl + func_repl + func_repl / var_repl;"),
        );
        // seqtool function arguments must not be nested further
        // (in `test_rewrite`, we don't return an error, just ignore the call)
        test_rewrite(
            "func(other(a, b), variable)",
            Err("Invalid function: 'func'. Seqtool function arguments must be strings or numbers."),
        );
    }

    #[test]
    fn loop_() {
        test_rewrite(
            "var a = variable - 10; for (var i = 0; i < variable; i++) \
                { a++; if (a == func('b')) { break; } }",
            Ok("var a = var_repl - 10; for (var i = 0; i < var_repl; i++) \
                { a++; if (a == func_repl) { break; } }"),
        );
    }

    #[test]
    fn functions() {
        test_rewrite("func('a', 'b')", Ok("func_repl"));
        // recognizes ...args as Fragment::Value
        let script = "var fn function(...args) {}";
        test_rewrite(script, Ok(script));
        // not a problem either
        let script = "var fn function({ a, b = 0 }) {}";
        test_rewrite(script, Ok(script));
        // arrow functions
        let script = "const avg = arg => arg;";
        test_rewrite(script, Ok(script));
        let script = "const avg = (...args) => args";
        test_rewrite(script, Ok(script));
        test_rewrite(
            "const avg = (...args) => { return args + variable; }",
            Ok("const avg = (...args) => { return args + var_repl; }"),
        );
    }

    #[test]
    fn strings_comments() {
        let script = "3 / 8; let a = \"as\\\"df\"; // comment\nvar b = 'asd\\'f' + `as\\`df`;/* comment\n\n*/ var c = 0;";
        test_rewrite(script, Ok(script));
        // escaped string
        test_rewrite(r" '[\'a\']' ", Ok(r" '[\'a\']' "));
        // with escaped backslash
        test_rewrite(r" '[\\\'a\']' ", Ok(r" '[\\\'a\']' "));
        // no end quote (since escaped)
        test_rewrite(r"         'a\' ", Err(r"'a\' "));
        // template string
        test_rewrite(
            "`abc ${variable} def ${ func('a', 'b')} \\${variable}`",
            Ok("`abc ${var_repl} def ${ func_repl} \\${variable}`"),
        );
    }

    #[test]
    fn invalid_syntax() {
        test_rewrite("func(a,}", Err("(a,}"));
        test_rewrite("a + b({x: [1, variable])};", Err("({x: [1, variable])};"));
    }

    #[test]
    fn regex() {
        // regex literals are not recognized
        // vars are replaced in regex as well (interpreting '/' as division)
        test_rewrite(
            "const re = /variable+func('a')/i;",
            Ok("const re = /var_repl+func_repl/i;"),
        );
        // the parsing can thus also fail
        test_rewrite("const re = /variable[/i;", Err("[/i;"));
    }

    #[cfg(feature = "expr")]
    #[test]
    fn varstring() {
        let vs = "a {{file:path.js }}b,{ func(c,'d','e')},_f,,raw('var'),{{ g/h(i, 'j') }} rest";
        use ParsedVarStringSegment::*;
        // allowing variables without braces
        let res = parse_varstring_list(vs, true);
        let exp = vec![
            vec![Text("a "), SourceFile("path.js "), Text("b")],
            vec![Var(Func::new(
                "func",
                &["c".to_string(), "d".to_string(), "e".to_string()],
            ))],
            vec![VarOrText(Func::new("_f", &[]), "_f")],
            vec![],
            vec![VarOrText(
                Func::new("raw", &["var".to_string()]),
                "raw('var')",
            )],
            vec![Expr(" g/h(i, 'j') "), Text(" rest")],
        ];
        // TODO: using unwrap() because comparing with Ok gives strange interaction with rkyv type
        assert_eq!(res.unwrap(), exp);
        // enforcing braces
        let res = parse_varstring_list(vs, false);
        let exp = vec![
            vec![Text("a "), SourceFile("path.js "), Text("b")],
            vec![Var(Func::new(
                "func",
                &["c".to_string(), "d".to_string(), "e".to_string()],
            ))],
            vec![Text("_f")],
            vec![],
            vec![Text("raw('var')")],
            vec![Expr(" g/h(i, 'j') "), Text(" rest")],
        ];
        assert_eq!(res.unwrap(), exp);
    }
}
