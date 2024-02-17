use std::cmp::min;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use seq_io::{
    fasta::{self, Record as FR},
    policy::BufPolicy,
};

use crate::config::SeqContext;
use crate::error::CliResult;

use super::{Record, SeqHeader, SeqLineIter, SeqReader, SeqWriter};

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

impl<R, P, O> SeqReader<O> for FaQualReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>> {
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
            Ok(func(&r))
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

impl<'a> Record for FaQualRecord<'a> {
    fn id_bytes(&self) -> &[u8] {
        self.fa_rec.id_bytes()
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.fa_rec.desc_bytes()
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        self.fa_rec.id_desc_bytes()
    }
    fn full_header(&self) -> SeqHeader {
        self.fa_rec.full_header()
    }
    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.fa_rec.header_delim_pos()
    }
    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.fa_rec.set_header_delim_pos(delim)
    }
    fn raw_seq(&self) -> &[u8] {
        self.fa_rec.raw_seq()
    }
    fn has_seq_lines(&self) -> bool {
        self.fa_rec.has_seq_lines()
    }
    fn qual(&self) -> Option<&[u8]> {
        Some(self.qual)
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
    pub fn new<Q>(wrap: Option<usize>, qual_path: Q) -> CliResult<Self>
    where
        Q: AsRef<Path>,
    {
        let q_handle = File::create(&qual_path).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Error creating '{}': {}",
                    qual_path.as_ref().to_string_lossy(),
                    e
                ),
            )
        })?;

        Ok(FaQualWriter {
            fa_writer: super::fasta::FastaWriter::new(wrap),
            qual_out: io::BufWriter::new(q_handle),
            wrap: wrap.unwrap_or(std::usize::MAX),
        })
    }
}

impl SeqWriter for FaQualWriter {
    fn write<W: io::Write>(
        &mut self,
        record: &dyn Record,
        ctx: &mut SeqContext,
        out: W,
    ) -> CliResult<()> {
        // write FASTA record
        self.fa_writer.write(record, ctx, out)?;

        // write quality scores
        let qual = record.qual().ok_or("No quality scores found in input.")?;

        // header
        match record.full_header() {
            SeqHeader::IdDesc(id, desc) => fasta::write_id_desc(&mut self.qual_out, id, desc)?,
            SeqHeader::FullHeader(h) => fasta::write_head(&mut self.qual_out, h)?,
        }

        // write Phred scores
        for qline in qual.chunks(self.wrap) {
            if !qline.is_empty() {
                let phred_qual = ctx.qual_converter.phred_scores(qline).map_err(|e| {
                    format!(
                        "Error writing record '{}'. {}",
                        String::from_utf8_lossy(record.id_bytes()),
                        e
                    )
                })?;
                let mut q_iter = phred_qual.scores().iter();
                for q in q_iter.by_ref().take(qline.len() - 1) {
                    write!(self.qual_out, "{} ", *q)?;
                }
                writeln!(self.qual_out, "{}", q_iter.next().unwrap())?;
            }
        }
        Ok(())
    }
}
