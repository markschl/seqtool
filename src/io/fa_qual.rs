use std::cmp::min;
use std::fs::File;
use std::io;
use std::path::Path;

use seq_io::{
    fasta::{self, Record as FR},
    policy::BufPolicy,
};

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::var::VarBuilder;

use super::output::FormatWriter;
use super::{Attribute, Record, RecordHeader, SeqLineIter, SeqReader};

// Reader

pub struct FaQualReader<R: io::Read, P: BufPolicy> {
    fa_rdr: fasta::Reader<R, P>,
    qual_rdr: fasta::Reader<File, P>,
    quals: Vec<u8>,
}

impl<R, P> FaQualReader<R, P>
where
    R: io::Read,
    P: BufPolicy + Clone,
{
    pub fn new<Q>(rdr: R, cap: usize, policy: P, qfile: Q) -> CliResult<Self>
    where
        Q: AsRef<Path>,
    {
        let qhandle = File::open(&qfile).map_err(|e| {
            format!(
                "Error opening '{}': {}",
                qfile.as_ref().to_string_lossy(),
                e
            )
        })?;

        Ok(FaQualReader {
            fa_rdr: fasta::Reader::with_capacity(rdr, cap).set_policy(policy.clone()),
            qual_rdr: fasta::Reader::with_capacity(qhandle, cap).set_policy(policy),
            quals: vec![],
        })
    }
}

impl<R, P> SeqReader for FaQualReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>> {
        let quals = &mut self.quals;
        let qual_rdr = &mut self.qual_rdr;

        self.fa_rdr.next().map(|rec| {
            let rec = rec?;

            // quality info
            quals.clear();
            let qrec = qual_rdr.next().ok_or_else(|| {
                format!(
                    "Quality scores in QUAL file missing for record '{}'",
                    String::from_utf8_lossy(rec.id_bytes())
                )
            })??;

            if qrec.id() != rec.id() {
                return fail!(format!(
                    "ID mismatch with QUAL file: '{}' != '{}'",
                    String::from_utf8_lossy(rec.id_bytes()),
                    String::from_utf8_lossy(qrec.id_bytes()),
                ));
            }

            for seq in qrec.seq_lines() {
                parse_quals(seq, quals)?;
            }

            // check sequence length
            // this may have a performance impact
            let seqlen = rec.seq_lines().fold(0, |l, seq| l + seq.len());

            if seqlen != quals.len() {
                return fail!(format!(
                    "The number of quality scores ({}) is not equal to sequence length ({}) in record '{}'",
                    quals.len(), seqlen,
                    String::from_utf8_lossy(rec.id_bytes()),
                ));
            }

            let r = FaQualRecord {
                fa_rec: super::fasta::FastaRecord::new(rec),
                qual: quals,
            };
            func(&r)
        })
    }
}

fn parse_quals(line: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    for qual in line.split(|c| *c == b' ') {
        let q = parse_int(qual).map_err(|_| {
            format!(
                "Invalid quality score found: '{}'",
                String::from_utf8_lossy(qual)
            )
        })?;
        out.push(min(q as u8, 255));
    }
    Ok(())
}

fn parse_int(bytes: &[u8]) -> Result<usize, ()> {
    if bytes.is_empty() {
        return Err(());
    }
    let mut out = 0;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return Err(());
        }
        out = 10 * out + (b - b'0') as usize;
    }
    Ok(out)
}

// Wrapper for FASTA record

pub struct FaQualRecord<'a> {
    fa_rec: super::fasta::FastaRecord<'a>,
    qual: &'a [u8],
}

impl Record for FaQualRecord<'_> {
    fn id(&self) -> &[u8] {
        self.fa_rec.id()
    }

    fn desc(&self) -> Option<&[u8]> {
        self.fa_rec.desc()
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        self.fa_rec.id_desc()
    }

    fn current_header(&self) -> RecordHeader {
        self.fa_rec.current_header()
    }

    fn raw_seq(&self) -> &[u8] {
        self.fa_rec.raw_seq()
    }

    fn qual(&self) -> Option<&[u8]> {
        Some(self.qual)
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.fa_rec.header_delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.fa_rec.set_header_delim_pos(delim)
    }

    fn has_seq_lines(&self) -> bool {
        self.fa_rec.has_seq_lines()
    }

    fn seq_segments(&self) -> SeqLineIter {
        self.fa_rec.seq_segments()
    }
}

// Writer

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
    // header
    out.write_all(b">")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;

    // Phred scores
    let qual = record.qual().ok_or("No quality scores found in input.")?;
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
