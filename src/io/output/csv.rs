use std::io;

use super::{Record, Writer};
use crate::var;
use crate::var::varstring;
use crate::{error::CliResult, var::varstring::register_var_list};

use csv;

pub struct CsvWriter<W: io::Write> {
    writer: csv::Writer<W>,
    field_list: String,
    compiled_fields: Vec<varstring::VarString>,
    row: Vec<Vec<u8>>,
}

impl<W: io::Write> CsvWriter<W> {
    pub fn new(writer: W, field_list: String, delim: u8) -> CsvWriter<W> {
        let writer = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(writer);
        CsvWriter {
            writer,
            field_list,
            compiled_fields: vec![],
            row: vec![],
        }
    }
}

impl<W: io::Write> Writer<W> for CsvWriter<W> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        self.compiled_fields.clear();
        self.row.clear();
        // progressively parse fields; this is necessary because there can be
        // commas in functions as well
        self.compiled_fields.clear();
        register_var_list(&self.field_list, ',', builder, &mut self.compiled_fields)?;
        self.row
            .extend((0..self.compiled_fields.len()).map(|_| vec![]));
        Ok(())
    }

    #[inline]
    fn has_vars(&self) -> bool {
        !self.compiled_fields.is_empty()
    }

    fn write(&mut self, record: &dyn Record, vars: &var::Vars) -> CliResult<()> {
        for (expr, parsed) in self.compiled_fields.iter().zip(&mut self.row) {
            parsed.clear();
            expr.compose(parsed, vars.symbols(), record);
        }
        self.writer.write_record(&self.row)?;
        Ok(())
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Some(self.writer.into_inner().map_err(Into::into))
    }
}
