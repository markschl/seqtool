use std::io;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::io::QualFormat;
use crate::var::VarBuilder;

use crate::io::{
    output::{fastx::register_attributes, FormatWriter},
    Record,
};

use super::fastx::Attribute;

pub struct FastqWriter {
    format: QualFormat,
}

impl FastqWriter {
    pub fn new(
        format: QualFormat,
        attrs: &[(Attribute, bool)],
        builder: &mut VarBuilder,
    ) -> CliResult<Self> {
        register_attributes(attrs, builder)?;
        Ok(Self { format })
    }
}

impl FormatWriter for FastqWriter {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        write_fastq(record, out, ctx, self.format)
    }
}

fn write_fastq<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    ctx: &mut SeqContext,
    format: QualFormat,
) -> CliResult<()> {
    // TODO: could use seq_io::fastq::write_to / write_parts, but the sequence is an iterator of segments
    let qual = record.qual().ok_or("No quality scores found in input.")?;

    // header
    out.write_all(b"@")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;

    // sequence
    for seq in record.seq_segments() {
        out.write_all(seq)?;
    }
    out.write_all(b"\n+\n")?;

    // quality scores
    let qual = ctx.qual_converter.convert_to(qual, format).map_err(|e| {
        format!(
            "Error writing record '{}'. {}",
            String::from_utf8_lossy(record.id()),
            e
        )
    })?;
    out.write_all(qual)?;
    out.write_all(b"\n")?;

    Ok(())
}
