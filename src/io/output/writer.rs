use var;
use io::Record;
use error::CliResult;

pub trait Writer {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()>;
    fn has_vars(&self) -> bool;
    fn write_simple(&mut self, record: &Record) -> CliResult<()>;
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()>;
}

impl<W: Writer + ?Sized> Writer for Box<W> {
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
}

// empty output

pub struct NoOutput;

impl Writer for NoOutput {
    fn register_vars(&mut self, _: &mut var::VarBuilder) -> CliResult<()> {
        Ok(())
    }
    fn has_vars(&self) -> bool {
        false
    }
    //fn prepare(&mut self, data: &var::Data) {}
    fn write_simple(&mut self, _: &Record) -> CliResult<()> {
        Ok(())
    }
    fn write(&mut self, _: &Record, _: &var::Vars) -> CliResult<()> {
        Ok(())
    }
}
