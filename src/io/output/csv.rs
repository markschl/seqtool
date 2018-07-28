use std::io;

use error::CliResult;
use var;
use var::varstring;

use io::SeqAttr;
use super::{Record, Writer};

use csv;

enum Field {
    SeqAttr(SeqAttr),
    Expr(varstring::VarString),
}

pub struct CsvWriter<W: io::Write> {
    writer: csv::Writer<W>,
    fields: Vec<String>,
    compiled_fields: Vec<Field>,
    row: Vec<Vec<u8>>,
}

impl<W: io::Write> CsvWriter<W> {
    pub fn new(writer: W, fields: Vec<String>, delim: u8) -> CsvWriter<W> {
        let writer = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(writer);
        CsvWriter {
            writer: writer,
            fields: fields,
            compiled_fields: vec![],
            row: vec![],
        }
    }
}

impl<W: io::Write> Writer<W> for CsvWriter<W> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        self.compiled_fields.clear();
        self.row.clear();
        for field in &self.fields {
            let compiled = match SeqAttr::from_str(field) {
                Some(a) => Field::SeqAttr(a),
                None => {
                    let expr = varstring::VarString::var_or_composed(field, builder)?;
                    Field::Expr(expr)
                }
            };
            self.compiled_fields.push(compiled);
            self.row.push(vec![]);
        }
        Ok(())
    }

    #[inline]
    fn has_vars(&self) -> bool {
        !self.fields.is_empty()
    }

    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        for (field, parsed) in self.compiled_fields.iter().zip(&mut self.row) {
            parsed.clear();
            match *field {
                Field::SeqAttr(attr) => record.write_attr(attr, parsed),
                Field::Expr(ref expr) => expr.compose(parsed, vars.symbols()),
            }
        }
        self.writer.write_record(&self.row)?;
        Ok(())
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Some(self.writer.into_inner().map_err(Into::into))
    }
}
