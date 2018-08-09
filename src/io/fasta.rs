
use std::io;
use std::borrow::{Cow,ToOwned};
use std::cell::Cell;

use memchr::memchr;

use error::CliResult;
use seq_io::BufStrategy;
use seq_io::fasta::{self, Record as FR, Reader};
use var;
use super::*;


// Reader

pub struct FastaReader<R: io::Read, S: BufStrategy>(pub Reader<R, S>);

impl<R, S> FastaReader<R, S>
    where
        R: io::Read,
        S: BufStrategy,
{
    pub fn new(rdr: R, cap: usize, strategy: S) -> Self {
        FastaReader(Reader::with_cap_and_strategy(rdr, cap, strategy))
    }
}

impl<R, S, O> SeqReader<O> for FastaReader<R, S>
    where
        R: io::Read,
        S: BufStrategy,
{
    fn read_next(&mut self, func: &mut FnMut(&Record) -> O) -> Option<CliResult<O>> {
        self.0.next().map(|r| {
            let r = FastaRecord::new(r?);
            Ok(func(&r))
        })
    }
}


// Wrapper for FASTA record

pub struct FastaRecord<'a> {
    rec: fasta::RefRecord<'a>,
    delim: Cell<Option<Option<usize>>>
}

impl<'a> FastaRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fasta::RefRecord<'a>) -> FastaRecord<'a> {
        FastaRecord {
            rec: inner,
            delim: Cell::new(None)
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

pub struct FastaWriter<W: io::Write> {
    io_writer: W,
    wrap: Option<usize>,
}

impl<W: io::Write> FastaWriter<W> {
    pub fn new(io_writer: W, wrap: Option<usize>) -> FastaWriter<W> {
        FastaWriter {
            io_writer: io_writer,
            wrap: wrap,
        }
    }
}


impl<W: io::Write> SeqWriter<W> for FastaWriter<W> {
    fn write(&mut self, record: &Record, _: &var::Vars) -> CliResult<()> {
        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => fasta::write_id_desc(&mut self.io_writer, id, desc)?,
            SeqHeader::FullHeader(h) => fasta::write_head(&mut self.io_writer, h)?,
        }
        if let Some(wrap) = self.wrap {
            fasta::write_wrap_seq_iter(&mut self.io_writer, record.seq_segments(), wrap)?;
        } else {
            fasta::write_seq_iter(&mut self.io_writer, record.seq_segments())?;
        }
        Ok(())
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Some(Ok(self.io_writer))
    }
}
