use std::io;

use super::WriteFinish;
use error::CliResult;
use io::Record;
use var;

pub trait Writer<W: io::Write> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()>;
    fn has_vars(&self) -> bool;
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()>;
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>>;
}

impl<Wr: Writer<W> + ?Sized, W: io::Write> Writer<W> for Box<Wr> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        (**self).register_vars(builder)
    }
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        (**self).write(record, vars)
    }
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        (*self).into_inner()
    }
}
