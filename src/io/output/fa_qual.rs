use std::fs::File;
use std::io;
use std::path::Path;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::var::VarBuilder;

use super::{Attribute, FormatWriter, Record};

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

impl FormatWriter for FaQualWriter {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        self.fa_writer.write(record, out, ctx)?;
        write_qscores(record, &mut self.qual_out, ctx, self.wrap)
    }
}

fn write_qscores<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    ctx: &mut SeqContext,
    wrap: usize,
) -> CliResult<()> {
    let qual = record.qual().ok_or("No quality scores found in input.")?;
    // header
    out.write_all(b">")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;

    // Phred scores
    for qline in qual.chunks(wrap) {
        if !qline.is_empty() {
            let phred_qual = ctx.qual_converter.phred_scores(qline).map_err(|e| {
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
