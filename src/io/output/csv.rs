use std::io;

use super::{Record, SeqFormatter};
use crate::config::SeqContext;
use crate::var::{varstring, VarBuilder};
use crate::{error::CliResult, var::varstring::register_var_list};

pub const DEFAULT_OUTFIELDS: &str = "id,desc,seq";

pub struct CsvWriter {
    delim: u8,
    fields: Vec<varstring::VarString>,
}

impl CsvWriter {
    pub fn new(field_list: &str, delim: u8, builder: &mut VarBuilder) -> CliResult<CsvWriter> {
        let mut out = Self {
            delim,
            fields: vec![],
        };

        // progressively parse fields; this is necessary because there can be
        // commas in functions as well
        register_var_list(field_list, builder, &mut out.fields, true, true)?;
        Ok(out)
    }
}

impl SeqFormatter for CsvWriter {
    // #[inline]
    // fn has_vars(&self) -> bool {
    //     !self.fields.is_empty()
    // }

    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        let mut is_first = true;
        for expr in &self.fields {
            if !is_first {
                write!(out, "{}", self.delim as char)?;
            }
            is_first = false;
            expr.compose(out, &ctx.symbols, record)?;
        }
        writeln!(out)?;
        Ok(())
    }
}
