use std::io;

use seq_io::fasta;

use crate::context::RecordMeta;
use crate::error::CliResult;
use crate::io::QualConverter;
use crate::var::VarBuilder;

use crate::io::{
    Record,
    output::{SeqFormatter, fastx::register_attributes},
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

impl SeqFormatter for FastaWriter {
    fn write_with(
        &mut self,
        record: &dyn Record,
        data: &RecordMeta,
        out: &mut dyn io::Write,
        _qc: &mut QualConverter,
    ) -> CliResult<()> {
        write_fasta(record, data, out, self.wrap)
    }
}

fn write_fasta<W: io::Write>(
    record: &dyn Record,
    data: &RecordMeta,
    mut out: W,
    wrap: Option<usize>,
) -> CliResult<()> {
    out.write_all(b">")?;
    data.attrs.write_head(record, &mut out, &data.symbols)?;
    out.write_all(b"\n")?;
    if let Some(w) = wrap {
        fasta::write_wrap_seq_iter(&mut out, record.seq_segments(), w)?;
    } else {
        fasta::write_seq_iter(&mut out, record.seq_segments())?;
    }
    Ok(())
}
