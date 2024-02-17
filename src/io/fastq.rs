use std::io;

use seq_io::fastq::{self, Reader, Record as FR};
use seq_io::policy::BufPolicy;

use crate::config::SeqContext;
use crate::error::CliResult;

use super::fastx::FastxHeaderParser;
use super::{QualFormat, Record, SeqHeader, SeqReader, SeqWriter};

// Reader

pub struct FastqReader<R: io::Read, P: BufPolicy>(pub Reader<R, P>);

impl<R, P> FastqReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    pub fn new(rdr: R, cap: usize, policy: P) -> Self {
        FastqReader(Reader::with_capacity(rdr, cap).set_policy(policy))
    }
}

impl<R, P, O> SeqReader<O> for FastqReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>> {
        self.0.next().map(|r| {
            let r = FastqRecord::new(r?);
            Ok(func(&r))
        })
    }
}

// Wrapper for FASTQ record

pub struct FastqRecord<'a> {
    rec: fastq::RefRecord<'a>,
    header_parser: FastxHeaderParser,
}

impl<'a> FastqRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fastq::RefRecord<'a>) -> FastqRecord<'a> {
        FastqRecord {
            rec: inner,
            header_parser: Default::default(),
        }
    }
}

impl<'a> Record for FastqRecord<'a> {
    fn id_bytes(&self) -> &[u8] {
        self.header_parser.id_desc(self.rec.head()).0
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.header_parser.id_desc(self.rec.head()).1
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        self.header_parser.id_desc(self.rec.head())
    }
    fn full_header(&self) -> SeqHeader {
        SeqHeader::FullHeader(self.rec.head())
    }
    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.header_parser.delim_pos()
    }
    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.header_parser.set_delim_pos(Some(delim))
    }
    fn raw_seq(&self) -> &[u8] {
        self.rec.seq()
    }
    fn has_seq_lines(&self) -> bool {
        false
    }
    fn qual(&self) -> Option<&[u8]> {
        Some(<fastq::RefRecord as fastq::Record>::qual(&self.rec))
    }
}

// Writer

pub struct FastqWriter {
    qual_fmt: QualFormat,
}

impl FastqWriter {
    pub fn new(qual_fmt: QualFormat) -> Self {
        Self { qual_fmt }
    }
}

impl SeqWriter for FastqWriter {
    fn write<W: io::Write>(
        &mut self,
        record: &dyn Record,
        ctx: &mut SeqContext,
        mut out: W,
    ) -> CliResult<()> {
        let qual = record.qual().ok_or("No quality scores found in input.")?;
        let qual = ctx
            .qual_converter
            .convert_to(qual, self.qual_fmt)
            .map_err(|e| {
                format!(
                    "Error writing record '{}'. {}",
                    String::from_utf8_lossy(record.id_bytes()),
                    e
                )
            })?;

        // TODO: could use seq_io::fastq::write_to / write_parts, but the sequence is an iterator of segments

        out.write_all(b"@")?;

        match record.full_header() {
            SeqHeader::FullHeader(h) => {
                out.write_all(h)?;
            }
            SeqHeader::IdDesc(id, desc) => {
                out.write_all(id)?;
                if let Some(d) = desc {
                    out.write_all(b" ")?;
                    out.write_all(d)?;
                }
            }
        }

        out.write_all(b"\n")?;
        for seq in record.seq_segments() {
            out.write_all(seq)?;
        }
        out.write_all(b"\n+\n")?;
        out.write_all(qual)?;
        out.write_all(b"\n")?;

        Ok(())
    }
}
