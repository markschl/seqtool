use std::cell::RefCell;
use std::str;

use regex;

use error::CliResult;
use var;

lazy_static! {
    static ref VAR_RE: regex::Regex =
        regex::Regex::new(r"(\{\{([^\}]+)\}\}|\{([^\{\}]+)\})").unwrap();
}

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
        let mut prev_pos = 0;

        for m in VAR_RE.find_iter(expr) {
            let var = m.as_str();
            let var_id = if var.starts_with("{{") {
                // math expression
                let expr = &var[2..var.len() - 2];
                vars.register_with_prefix(Some("expr_"), expr)?
            } else {
                // regular variable
                let name = &var[1..var.len() - 1];
                vars.register_var(name)?
            };
            let str_before = expr[prev_pos..m.start()].as_bytes().to_owned();
            outvars.push((str_before, var_id));
            prev_pos = m.end();
        }

        let rest = expr[prev_pos..].as_bytes().to_owned();

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
        str::from_utf8(&*string)?
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
        str::from_utf8(&*string)?
            .parse()
            .map_err(From::from)
            .map(Some)
    }
}
