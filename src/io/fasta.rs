use std::cell::Cell;
use std::io;

use memchr::memchr;
use seq_io::fasta::{self, Reader, Record as FR};
use seq_io::policy::BufPolicy;

use super::*;
use crate::error::CliResult;
use crate::var;

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
    delim: Cell<Option<Option<usize>>>,
}

impl<'a> FastaRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fasta::RefRecord<'a>) -> FastaRecord<'a> {
        FastaRecord {
            rec: inner,
            delim: Cell::new(None),
        }
    }

    #[inline(always)]
    fn _get_header(&self) -> (&[u8], Option<&[u8]>) {
        if let Some(d) = self.delim.get() {
            if let Some(d) = d {
                let (id, desc) = self.rec.head().split_at(d);
                (id, Some(&desc[1..]))
            } else {
                (self.rec.head(), None)
            }
        } else {
            self.delim.set(Some(memchr(b' ', self.rec.head())));
            self._get_header()
        }
    }
}

impl<'a> Record for FastaRecord<'a> {
    fn id_bytes(&self) -> &[u8] {
        self._get_header().0
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self._get_header().1
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        self._get_header()
    }
    fn delim(&self) -> Option<Option<usize>> {
        self.delim.get()
    }
    fn set_delim(&self, delim: Option<usize>) {
        self.delim.set(Some(delim))
    }
    fn get_header(&self) -> SeqHeader {
        SeqHeader::FullHeader(self.rec.head())
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
    fn write<W: io::Write>(&mut self, record: &dyn Record, _vars: &var::Vars, mut out: W) -> CliResult<()> {
        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => fasta::write_id_desc(&mut out, id, desc)?,
            SeqHeader::FullHeader(h) => fasta::write_head(&mut out, h)?,
        }
        if let Some(wrap) = self.wrap {
            fasta::write_wrap_seq_iter(&mut out, record.seq_segments(), wrap)?;
        } else {
            fasta::write_seq_iter(&mut out, record.seq_segments())?;
        }
        Ok(())
    }
}
