use std::io;

use seq_io::fasta;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::var::VarBuilder;

use crate::io::{
    output::{fastx::register_attributes, FormatWriter},
    Record,
};

use super::fastx::Attribute;

pub struct FastaWriter {
    wrap: Option<usize>,
}

impl FastaWriter {
    pub fn new(
        wrap: Option<usize>,
        attrs: &[(Attribute, bool)],
        builder: &mut VarBuilder,
    ) -> CliResult<Self> {
        register_attributes(attrs, builder)?;
        Ok(Self { wrap })
    }
}

impl FormatWriter for FastaWriter {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        write_fasta(record, out, ctx, self.wrap)
    }
}

fn write_fasta<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    ctx: &mut SeqContext,
    wrap: Option<usize>,
) -> CliResult<()> {
    out.write_all(b">")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;
    if let Some(w) = wrap {
        fasta::write_wrap_seq_iter(&mut out, record.seq_segments(), w)?;
    } else {
        fasta::write_seq_iter(&mut out, record.seq_segments())?;
    }
    Ok(())
}
