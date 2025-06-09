use std::io;

use seq_io::fastq::{self, Reader, Record as _};
use seq_io::policy::BufPolicy;

use crate::error::CliResult;
use crate::io::{MaybeModified, Record, RecordHeader};

use super::fastx::FastxHeaderParser;
use super::SeqReader;

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

impl<R, P> SeqReader for FastqReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next_conditional(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>> {
        self.0.next().map(|r| {
            let r = FastqRecord::new(r?);
            func(&r)
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

impl Record for FastqRecord<'_> {
    fn id(&self) -> &[u8] {
        self.header_parser.id_desc(self.rec.head()).0
    }

    fn desc(&self) -> Option<&[u8]> {
        self.header_parser.id_desc(self.rec.head()).1
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        self.header_parser.id_desc(self.rec.head())
    }

    fn current_header(&self) -> RecordHeader {
        if let Some((id, desc)) = self.header_parser.parsed_id_desc(self.rec.head()) {
            RecordHeader::IdDesc(
                MaybeModified::new(id, false),
                MaybeModified::new(desc, false),
            )
        } else {
            RecordHeader::Full(self.rec.head())
        }
    }

    fn raw_seq(&self) -> &[u8] {
        self.rec.seq()
    }

    fn qual(&self) -> Option<&[u8]> {
        Some(<fastq::RefRecord as fastq::Record>::qual(&self.rec))
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.header_parser.delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.header_parser.set_delim_pos(Some(delim))
    }
}
