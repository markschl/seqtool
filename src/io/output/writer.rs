use std::io;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::io::Record;

pub trait FormatWriter {
    // fn has_vars(&self) -> bool;
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()>;
}

impl<W: FormatWriter + ?Sized> FormatWriter for Box<W> {
    // fn has_vars(&self) -> bool {
    //     (**self).has_vars()
    // }
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        (**self).write(record, out, ctx)
    }
}
