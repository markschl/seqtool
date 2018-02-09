
use std::io;

use var;
use io::Record;
use error::CliResult;
use super::WriteFinish;


pub trait Writer<W: io::Write> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()>;
    fn has_vars(&self) -> bool;
    fn write_simple(&mut self, record: &Record) -> CliResult<()>;
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
    fn write_simple(&mut self, record: &Record) -> CliResult<()> {
        (**self).write_simple(record)
    }
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        (**self).write(record, vars)
    }
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        (*self).into_inner()
    }
}

// empty output

pub struct NoOutput;

impl<W: io::Write> Writer<W> for NoOutput {
    fn register_vars(&mut self, _: &mut var::VarBuilder) -> CliResult<()> {
        Ok(())
    }
    fn has_vars(&self) -> bool {
        false
    }
    fn write_simple(&mut self, _: &Record) -> CliResult<()> {
        Ok(())
    }
    fn write(&mut self, _: &Record, _: &var::Vars) -> CliResult<()> {
        Ok(())
    }
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        None
    }
}
