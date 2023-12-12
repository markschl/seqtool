use std::io;

use crate::error::CliResult;
use crate::io::Record;
use crate::var;

pub trait FormatWriter {
    fn has_vars(&self) -> bool;
    fn write(&mut self, record: &dyn Record, out: &mut dyn io::Write, vars: &var::Vars) -> CliResult<()>;
}

impl<W: FormatWriter + ?Sized> FormatWriter for Box<W> {
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn write(&mut self, record: &dyn Record, out: &mut dyn io::Write, vars: &var::Vars) -> CliResult<()> {
        (**self).write(record, out, vars)
    }
}
