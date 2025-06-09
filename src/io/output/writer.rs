use std::io;

use crate::context::{RecordMeta, SeqContext};
use crate::error::CliResult;
use crate::io::{QualConverter, Record};

pub trait SeqFormatter {
    /// Write a formatted record to `out`, given the metadata in `ctx`.
    /// This is a convenience wrapper around `write_with`, which allows directly
    /// providing `SeqContext`.
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        self.write_with(record, &ctx.meta[0], out, &mut ctx.qual_converter)
    }

    /// Write a formatted record to `out`, given all necessary metadata.
    fn write_with(
        &mut self,
        record: &dyn Record,
        data: &RecordMeta,
        out: &mut dyn io::Write,
        qc: &mut QualConverter,
    ) -> CliResult<()>;
}

impl<W: SeqFormatter + ?Sized> SeqFormatter for Box<W> {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        (**self).write(record, out, ctx)
    }

    fn write_with(
        &mut self,
        record: &dyn Record,
        data: &RecordMeta,
        out: &mut dyn io::Write,
        qc: &mut QualConverter,
    ) -> CliResult<()> {
        (**self).write_with(record, data, out, qc)
    }
}
