use std::fs::File;
use std::io;
use std::path::Path;

use crate::context::RecordMeta;
use crate::error::CliResult;
use crate::io::QualConverter;
use crate::var::VarBuilder;

use super::{Attribute, Record, SeqFormatter};

pub struct FaQualWriter {
    fa_writer: super::fasta::FastaWriter,
    // This is a bit awkward: the FASTA writer is not part of this struct,
    // (supplied to write(), while the .qual writer is).
    // However, this is a special case and not a problem.
    qual_out: io::BufWriter<File>,
    wrap: usize,
}

impl FaQualWriter {
    pub fn new<Q>(
        wrap: Option<usize>,
        qual_path: Q,
        attrs: &[(Attribute, bool)],
        builder: &mut VarBuilder,
    ) -> CliResult<Self>
    where
        Q: AsRef<Path>,
    {
        let q_handle = File::create(&qual_path).map_err(|e| {
            io::Error::other(format!(
                "Error creating '{}': {}",
                qual_path.as_ref().to_string_lossy(),
                e
            ))
        })?;

        Ok(FaQualWriter {
            fa_writer: super::fasta::FastaWriter::new(wrap, attrs, builder)?,
            qual_out: io::BufWriter::new(q_handle),
            wrap: wrap.unwrap_or(usize::MAX),
        })
    }
}

impl SeqFormatter for FaQualWriter {
    fn write_with(
        &mut self,
        record: &dyn Record,
        data: &RecordMeta,
        out: &mut dyn io::Write,
        qc: &mut QualConverter,
    ) -> CliResult<()> {
        self.fa_writer.write_with(record, data, out, qc)?;
        write_qscores(record, &mut self.qual_out, data, qc, self.wrap)
    }
}

fn write_qscores<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    data: &RecordMeta,
    qual_converter: &mut QualConverter,
    wrap: usize,
) -> CliResult<()> {
    let qual = record.qual().ok_or("No quality scores found in input.")?;
    // header
    out.write_all(b">")?;
    data.attrs.write_head(record, &mut out, &data.symbols)?;
    out.write_all(b"\n")?;

    // Phred scores
    for qline in qual.chunks(wrap) {
        if !qline.is_empty() {
            let phred_qual = qual_converter.phred_scores(qline).map_err(|e| {
                format!(
                    "Error writing record '{}'. {}",
                    String::from_utf8_lossy(record.id()),
                    e
                )
            })?;
            let mut q_iter = phred_qual.scores().iter();
            for q in q_iter.by_ref().take(qline.len() - 1) {
                write!(&mut out, "{} ", *q)?;
            }
            writeln!(&mut out, "{}", q_iter.next().unwrap())?;
        }
    }
    Ok(())
}
