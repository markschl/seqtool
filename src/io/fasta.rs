use std::io;

use seq_io::fasta::{self, Reader, Record as SeqioRecord};
use seq_io::policy::BufPolicy;

use crate::config::SeqContext;
use crate::error::CliResult;

use super::fastx::FastxHeaderParser;
use super::{Record, SeqHeader, SeqLineIter, SeqReader, SeqWriter};

// Reader

pub struct FastaReader<R: io::Read, P: BufPolicy>(pub Reader<R, P>);

impl<R, P> FastaReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    pub fn new(rdr: R, cap: usize, policy: P) -> Self {
        FastaReader(Reader::with_capacity(rdr, cap).set_policy(policy))
    }
}

impl<R, P, O> SeqReader<O> for FastaReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>> {
        self.0.next().map(|r| {
            let r = FastaRecord::new(r?);
            Ok(func(&r))
        })
    }
}

// Wrapper for FASTA record

pub struct FastaRecord<'a> {
    rec: fasta::RefRecord<'a>,
    header_parser: FastxHeaderParser,
}

impl<'a> FastaRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fasta::RefRecord<'a>) -> FastaRecord<'a> {
        FastaRecord {
            rec: inner,
            header_parser: Default::default(),
        }
    }
}

impl<'a> Record for FastaRecord<'a> {
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
        self.rec.num_seq_lines() > 1
    }
    fn qual(&self) -> Option<&[u8]> {
        None
    }
    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Fasta(self.rec.seq_lines())
    }
}

// Writer

pub struct FastaWriter {
    wrap: Option<usize>,
}

impl FastaWriter {
    pub fn new(wrap: Option<usize>) -> Self {
        Self { wrap }
    }
}

impl SeqWriter for FastaWriter {
    fn write<W: io::Write>(
        &mut self,
        record: &dyn Record,
        _: &mut SeqContext,
        mut out: W,
    ) -> CliResult<()> {
        match record.full_header() {
            SeqHeader::FullHeader(h) => fasta::write_head(&mut out, h)?,
            SeqHeader::IdDesc(id, desc) => fasta::write_id_desc(&mut out, id, desc)?,
        }
        if let Some(wrap) = self.wrap {
            fasta::write_wrap_seq_iter(&mut out, record.seq_segments(), wrap)?;
        } else {
            fasta::write_seq_iter(&mut out, record.seq_segments())?;
        }
        Ok(())
    }
}
