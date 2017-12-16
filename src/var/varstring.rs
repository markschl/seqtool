extern crate nom;

use std::cell::RefCell;

use nom::IResult;

use var;
use error::CliResult;

#[derive(Debug)]
enum VarType {
    Var,
    Expr,
}

named!(_var(&str) -> (&str, Option<VarType>),
    do_parse!(
        tag!("{") >>
        v: take_until!("}") >>
        take!(1) >>
        (v, Some(VarType::Var))
    )
);

named!(_expr(&str) -> (&str, Option<VarType>),
    do_parse!(
        tag!("{{") >>
        v: take_until!("}}") >>
        take!(2) >>
        (v, Some(VarType::Expr))
    )
);

named!(find_vars(&str) -> (&str, Option<VarType>),
    alt!(_expr | _var | do_parse!(take!(1) >> ("", None)))
);

#[derive(Debug)]
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
    pub fn var_or_composed(text: &str, vars: &mut var::VarBuilder) -> CliResult<VarString> {
        let res = vars.register_var(text);
        Ok(match res {
            Ok(id) => VarString {
                parts: vec![(vec![], id)],
                rest: vec![],
                one_var: true,
                num_string: RefCell::new(vec![]),
            },
            Err(_) => VarString::parse_register(text, vars)?,
        })
    }

    /// Parses a string containing variables in the form " {varname} "
    pub fn parse_register(expr: &str, vars: &mut var::VarBuilder) -> CliResult<VarString> {
        let mut outvars = vec![];

        let mut pos = 0;
        let mut prev_pos = 0;
        let mut rest = &expr[..];

        while let IResult::Done(_rest, (value, ty)) = find_vars(rest) {
            rest = _rest;
            if let Some(ty) = ty {
                let str_before = expr[prev_pos..pos].as_bytes().to_owned();
                match ty {
                    VarType::Var => {
                        let id = vars.register_var(value)?;
                        outvars.push((str_before, id));
                    }
                    VarType::Expr => {
                        let id = vars.register_with_prefix(Some("expr_"), value)?;
                        outvars.push((str_before, id));
                    }
                }
                prev_pos = expr.len() - _rest.len();
            }
            pos = expr.len() - _rest.len();
        }

        let rest = expr[prev_pos..expr.len()].as_bytes().to_owned();
        let one_var = outvars.len() == 1 && outvars[0].0.is_empty() && rest.is_empty();
        Ok(VarString {
            parts: outvars,
            rest: rest,
            one_var: one_var,
            num_string: RefCell::new(vec![]),
        })
    }

    /// Caution: the string is not cleared, any data is appended! clear it by yourself if needed
    #[inline]
    pub fn compose(&self, out: &mut Vec<u8>, table: &var::symbols::Table) {
        for &(ref str_before, id) in &self.parts {
            out.extend_from_slice(str_before);
            out.extend_from_slice(table.get_text(id).unwrap_or(b""));
        }
        out.extend_from_slice(&self.rest);
    }

    #[inline]
    pub fn get_float(&self, table: &var::symbols::Table) -> CliResult<Option<f64>> {
        if self.one_var {
            return table.get_float(self.parts[0].1);
        }
        let mut string = self.num_string.borrow_mut();
        string.clear();
        self.compose(&mut string, table);
        if string.len() == 0 {
            return Ok(None);
        }
        ::std::str::from_utf8(&*string)?
            .parse()
            .map_err(From::from)
            .map(Some)
    }

    #[inline]
    pub fn get_int(&self, table: &var::symbols::Table) -> CliResult<Option<i64>> {
        if self.one_var {
            return table.get_int(self.parts[0].1);
        }
        let mut string = self.num_string.borrow_mut();
        string.clear();
        self.compose(&mut string, table);
        if string.len() == 0 {
            return Ok(None);
        }
        ::std::str::from_utf8(&*string)?
            .parse()
            .map_err(From::from)
            .map(Some)
    }
}
