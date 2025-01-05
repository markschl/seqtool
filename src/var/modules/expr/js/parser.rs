//! Simple JavaScript source parser for recognizing variables/functions
//! and replacing them with placeholder variables.
//! It *should* not fail with any valid JavaScript (otherwise there is a bug),
//! except for some cases described in the `PseudoAst` documentation,
//! such as literal /regexp/ notation.

use std::fmt;
use std::fs::read_to_string;

use phf::{phf_set, Set};
use winnow::ascii::{multispace0, multispace1};
use winnow::combinator::{alt, delimited, opt, preceded, repeat};
use winnow::stream::Location;
use winnow::token::{one_of, take_till, take_until};
use winnow::{Located, PResult, Parser};

use crate::var::parser::{
    name, some_value, string, string_fragment, var_or_func, Arg, LocatedStream, StrStream, VarFunc,
    VarStringParseErr,
};

/// list of JavaScript keywords that cannot be variable names
static BUILTINS: Set<&'static str> = phf_set! {
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
};

impl std::fmt::Display for VarStringParseErr {
    #[cold]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Failed to parse the following string with variables/functions or JavaScript code: \
            \n`{}`\n\
            Make sure to correctly close all parentheses and {{ brackets }} \
            indicating a variable/function or JavaScript expression. \
            e.g. {{ id }} or {{ attr('a') }} or {{ seq_num + 1 }}, or {{ file:path/to/script.js }}. \
            Avoid Javascript /regex/ literals (use `new RegExp(\"regex\")` instead). \
            General Javascript help: \
            https://developer.mozilla.org/en-US/docs/Web/JavaScript/Language_overview \
            ",
            self.0
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression<'a> {
    Expr(SimpleAst<'a>),
    SourceFile(String),
}

impl fmt::Display for Expression<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Expr(expr) => write!(f, "{}", expr.script),
            Expression::SourceFile(path) => write!(f, "file:{}", path),
        }
    }
}

impl<'a> Expression<'a> {
    pub fn parse(script: &'a str) -> Result<Self, VarStringParseErr> {
        let mut located = Located::new(script);
        let expr = expression
            .parse_next(&mut located)
            .map_err(|_| VarStringParseErr(located.to_string()))?;
        if located.len() > 0 {
            return Err(VarStringParseErr(located.to_string()));
        }
        Ok(expr)
    }

    pub fn with_tree<F, O>(&self, mut func: F) -> Result<O, String>
    where
        F: FnMut(&SimpleAst) -> O,
    {
        match self {
            Expression::Expr(t) => Ok(func(t)),
            Expression::SourceFile(path) => {
                let script =
                    read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
                let ast = SimpleAst::from_script(&script).map_err(|e| e.to_string())?;
                Ok(func(&ast))
            }
        }
    }
}

/// Simplified "AST" representation of a script, which is tailored
/// towards approximately (but correctly) parsing the JavaScript syntax
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
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleAst<'a> {
    pub script: &'a str,
    pub tree: Fragment<'a>,
}

impl<'a> SimpleAst<'a> {
    /// Parses a script into a simplified AST, optimized for simplicity rather
    /// than speed or complex syntax recognition.
    /// Does *not* recognize /regex notation/, since it is difficult to parse.
    /// Returns Err(rest) if only part of the script was parsed.
    pub fn from_script(script: &'a str) -> Result<Self, VarStringParseErr> {
        let mut input = script;
        let ast = expr
            .parse_next(&mut input)
            .map_err(|_| VarStringParseErr(input.to_string()))?;
        // dbg!(input, &ast);
        if !input.is_empty() {
            return Err(VarStringParseErr(input.to_string()));
        }
        Ok(ast)
    }

    /// Extracts functions/variables from the script, calling `reg_fn` on every of those.
    /// `reg_fn` should return the name of the placeholder variable, which is then
    /// defined by 'seqtool' (functions are all replaced by static variables).
    /// Returns the rewritten script.
    /// `reg_fn` may return `None` if the variable/function is unknown
    /// (meaning that it is likely a JavaScript function) or Some(Err("..."))
    /// if the arguments are invalid.
    pub fn rewrite<F>(&self, mut register_fn: F) -> Result<String, String>
    where
        F: FnMut(&VarFunc) -> Result<Option<String>, String>,
    {
        let mut vars = Vec::new();
        _collect_functions(&self.tree, &mut register_fn, &mut vars)?;
        // sort by order of occurrence
        vars.sort_by_key(|(v, _)| v.range.start);
        // rewrite the script sequentially
        let mut out = String::with_capacity(self.script.len());
        let mut prev_end = 0;
        // TODO: cannot use `replace_iter` because the replacements would not be accessible
        for (func, replacement) in vars {
            debug_assert!(func.range.start >= prev_end);
            out.push_str(&self.script[prev_end..func.range.start]);
            out.push_str(&replacement);
            prev_end = func.range.end;
        }
        out.push_str(&self.script[prev_end..]);
        Ok(out)
    }
}

/// Collects all variables/functions from the pseudo AST into a vector of
/// (range, name, argument) tuples
fn _collect_functions<'a, F>(
    node: &Fragment<'a>,
    reg_fn: &mut F,
    out: &mut Vec<(VarFunc<'a>, String)>,
) -> Result<(), String>
where
    F: FnMut(&VarFunc) -> Result<Option<String>, String>,
{
    match node {
        Fragment::Func(func) => {
            if let Some(replacement) = reg_fn(func)? {
                out.push((func.clone(), replacement));
            } else {
                for arg in func.args.as_deref().unwrap_or(&[]) {
                    if let Arg::Func(func) = arg {
                        _collect_functions(&Fragment::Func(func.clone()), reg_fn, out)?;
                    }
                }
            }
        }
        // All remaining function-like constructs and nested blocks that are
        // clearly not 'seqtool-like' functions
        Fragment::FuncLike { args: nested, .. } | Fragment::Nested(nested) => {
            for item in nested {
                _collect_functions(item, reg_fn, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Variable/function string parser
pub fn expression<'a, S: StrStream<'a>>(input: &mut S) -> PResult<Expression<'a>> {
    alt((
        // { file:path/to/script.js }
        preceded(
            "file:",
            alt((
                string.map(|s| s.to_string()),
                take_until(1.., "}").map(|s: &str| s.to_string()),
            )),
        )
        .map(|s| Expression::SourceFile(s.trim().to_string())),
        // { expression }
        expr.map(Expression::Expr),
    ))
    .parse_next(input)
}

/// Parses a script into a simplified AST, optimized for small binary size rather
/// than speed or complex syntax recognition.
/// Does *not* recognize /regex notation/, since it is difficult to parse.
/// Returns Err(rest) if only part of the script was parsed.
pub fn expr<'a, S: StrStream<'a>>(input: &mut S) -> PResult<SimpleAst<'a>> {
    let mut located = Located::new(input.clone());
    let (mut frags, script) = statements.with_taken().parse_next(&mut located)?;
    input.next_slice(located.location());
    let tree = if frags.len() == 1 {
        frags.pop().unwrap()
    } else {
        Fragment::Nested(frags)
    };
    Ok(SimpleAst { script, tree })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fragment<'a> {
    /// reserved/builtin keyword or variable/function/class name
    Identifier(&'a str),
    /// any kind of operator
    Operator(char),
    /// Functions/identifiers that match the 'seqtool-style' variable/functions
    /// syntax
    /// (we don't know this for sure until we try to register them)
    Func(VarFunc<'a>),
    // Other function calls or definitions, object construction
    // or member access (we use '.' as function name there)
    FuncLike {
        name: &'a str,
        args: Vec<Fragment<'a>>,
    },
    // numeric and string literals,
    Literal(&'a str),
    // list of statements in a block scope, template string parts,
    // array elements, etc.
    Nested(Vec<Fragment<'a>>),
}

/// "statement" (sequence of 'item' terminated by semicolon or comma;
/// even though these are not generally interchangeable
fn stmt<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Option<Fragment<'a>>> {
    item_terminated(&[';', ',']).parse_next(input)
}

/// List of "statements" (without ignored parts)
#[inline(never)]
fn statements<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Fragment<'a>>> {
    repeat(.., stmt)
        .fold(Vec::new, |mut acc: Vec<_>, frag| {
            if let Some(f) = frag {
                acc.push(f)
            }
            acc
        })
        .parse_next(input)
}

fn item_terminated<'a, S: LocatedStream<'a>>(
    delim: &[char],
) -> impl FnMut(&mut S) -> PResult<Option<Fragment<'a>>> + '_ {
    move |input: &mut S| {
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

fn opt_item<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Option<Fragment<'a>>> {
    alt((item, multispace0.map(|_| None))).parse_next(input)
}

/// Javascript "items" (keywords, variables, operators, blocks, function calls, arrays, etc)
#[inline(never)]
fn item<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Option<Fragment<'a>>> {
    use Fragment::*;
    alt((
        alt((multispace1, comment, inline_comment)).map(|_| None),
        alt((
            block.map(Nested),
            index_list.map(Nested),
            parens_list.map(Nested),
            operator.map(Operator),
            string.take().map(Literal),
            template_string.map(Nested),
            var_or_func.verify(|f| !BUILTINS.contains(f.name)).map(Func),
            func.map(|(name, args)| FuncLike { name, args }),
            name.map(Identifier),
            // regex.with_span().map(|(r, s)| Regex(r, s)),
            member_access,
            some_value.map(Literal),
        ))
        .map(Some),
    ))
    .parse_next(input)
}

// block scopes (function body, class definition, etc.), object notation
fn block<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Fragment<'a>>> {
    delimited('{', statements, '}').parse_next(input)
}

// arrays: [a, b, c], array indexing, etc.
// For simplicity, we allow semicolons as well, even though they are invalid in JS
fn index_list<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Fragment<'a>>> {
    delimited('[', statements, ']').parse_next(input)
}

// parens in math expressions, function arguments, etc.
fn parens_list<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Fragment<'a>>> {
    delimited('(', statements, ')').parse_next(input)
}

// .member or .member_fn()
fn member_access<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Fragment<'a>> {
    preceded(
        ".",
        alt((
            func.map(|(name, args)| Fragment::FuncLike {
                name: ".",
                args: vec![Fragment::FuncLike { name, args }],
            }),
            name.map(Fragment::Identifier),
        )),
    )
    .parse_next(input)
}

// func_name(a, b) or if/for/while, etc.
#[inline(never)]
fn func<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<(&'a str, Vec<Fragment<'a>>)> {
    (name, parens_list).parse_next(input)
}

/// Operators and operator-like characters (including => arrow function definition).
/// We only match single characters, thus operators such as '!=' and '>>>' will just
/// result in a sequence of single operators
fn operator<'a, S: StrStream<'a>>(input: &mut S) -> PResult<char> {
    one_of([
        '=', '!', '?', ':', '&', '|', '<', '>', '+', '-', '*', '/', '%', '^', '~',
    ])
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

// `template string with ${vars}, or \${escaped}`
// note: these are *not* valid as seqtool string arguments, and may contain
// of several 'nested' parts
fn template_string<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Vec<Fragment<'a>>> {
    delimited('`', repeat(.., template_fragment), '`').parse_next(input)
}

fn template_fragment<'a, S: LocatedStream<'a>>(input: &mut S) -> PResult<Fragment<'a>> {
    alt((
        preceded('$', block).map(Fragment::Nested),
        "$".map(Fragment::Literal),
        string_fragment(&['`', '$']).map(Fragment::Literal),
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
        let res = SimpleAst::from_script(script)
            .map_err(|e| e.to_string())
            .and_then(|ast| {
                ast.rewrite(|func| {
                    let rep = match func.name {
                        "variable" => "var_repl".to_string(),
                        "func" => "func_repl".to_string(),
                        _ => return Ok(None),
                    };
                    Ok(Some(rep))
                })
            });
        match res {
            Ok(out) => {
                assert!(expected.is_ok());
                assert_eq!(out, expected.unwrap());
            }
            Err(e) => {
                assert!(expected.is_err());
                // dbg!(expected, format!("`{}`", e));
                assert!(e.contains(&format!("`{}`", expected.unwrap_err())));
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
        // nested arguments are allowed
        test_rewrite("func(other(a, b), variable)", Ok("func_repl"));
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
        use crate::var::parser::Arg;
        use crate::var::parser::{parse_varstring_list, ParsedVarStringSegment::*};
        // TODO: test/fix string quote escapes
        let vs = r"a {file: path with spaces.js }b,{ func(c,'d','e')},_f,,num(g(a)),raw('var'),{ num(g)/h(i, 'j')+f(['ab']) } rest";
        // parse allowing variables/functions without { braces } in the comma-delimited list
        let res = parse_varstring_list(vs, true);
        // JS expression within the variable string
        let num_ast = SimpleAst {
            script: " num(g)/h(i, 'j')+f(['ab']) ",
            tree: Fragment::Nested(vec![
                Fragment::Func(VarFunc::new(
                    "num",
                    Some(vec![Arg::Func(VarFunc::new("g", None, 5..6))]),
                    1..7,
                )),
                Fragment::Operator('/'),
                Fragment::Func(VarFunc::new(
                    "h",
                    Some(vec![
                        Arg::Func(VarFunc::new("i", None, 10..11)),
                        Arg::Str("j".into()),
                    ]),
                    8..17,
                )),
                Fragment::Operator('+'),
                Fragment::FuncLike {
                    name: "f",
                    args: vec![Fragment::Nested(vec![Fragment::Literal("'ab'")])],
                },
            ]),
        };
        let exp = vec![
            vec![
                Text("a "),
                Expr(Expression::SourceFile("path with spaces.js".to_string())),
                Text("b"),
            ],
            vec![Var(VarFunc::new(
                "func",
                Some(vec![
                    Arg::Func(VarFunc::new("c", None, 39..40)),
                    "d".into(),
                    "e".into(),
                ]),
                34..49,
            ))],
            vec![VarOrText {
                func: VarFunc::new("_f", None, 51..53),
                text: "_f",
            }],
            vec![],
            vec![VarOrText {
                func: VarFunc::new(
                    "num",
                    Some(vec![Arg::Func(VarFunc::new(
                        "g",
                        Some(vec![Arg::Func(VarFunc::new("a", None, 61..62))]),
                        59..63,
                    ))]),
                    55..64,
                ),
                text: "num(g(a))",
            }],
            vec![VarOrText {
                func: VarFunc::new("raw", Some(vec!["var".into()]), 65..75),
                text: "raw('var')",
            }],
            vec![Expr(Expression::Expr(num_ast.clone())), Text(" rest")],
        ];
        assert_eq!(res.unwrap(), exp);
        // enforcing { braces } around every item
        let res = parse_varstring_list(vs, false);
        let exp = vec![
            vec![
                Text("a "),
                Expr(Expression::SourceFile("path with spaces.js".to_string())),
                Text("b"),
            ],
            vec![Var(VarFunc::new(
                "func",
                Some(vec![
                    Arg::Func(VarFunc::new("c", None, 39..40)),
                    "d".into(),
                    "e".into(),
                ]),
                34..49,
            ))],
            vec![Text("_f")],
            vec![],
            vec![Text("num(g(a))")],
            vec![Text("raw('var')")],
            vec![Expr(Expression::Expr(num_ast.clone())), Text(" rest")],
        ];
        assert_eq!(res.unwrap(), exp);
    }
}
