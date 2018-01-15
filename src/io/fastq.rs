
use std::io;
use error::CliResult;
use std::borrow::Cow;
use std::cell::Cell;

use memchr::memchr;

use seq_io::fastq::{self, Record as FR};
use super::*;

// Wrapper for FASTQ record

pub struct FastqRecord<'a> {
    rec: fastq::RefRecord<'a>,
    delim: Cell<Option<Option<usize>>>
}

impl<'a> FastqRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fastq::RefRecord<'a>) -> FastqRecord<'a> {
        FastqRecord {
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

impl<'a> Record for FastqRecord<'a> {
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
        false
    }
    fn qual(&self) -> Option<&[u8]> {
        Some(<fastq::RefRecord as fastq::Record>::qual(&self.rec))
    }
}


// Writer

pub struct FastqWriter<W: io::Write>(W);

impl<W: io::Write> FastqWriter<W> {
    pub fn new(io_writer: W) -> FastqWriter<W> {
        FastqWriter(io_writer)
    }
}

impl<W: io::Write> SeqWriter for FastqWriter<W> {
    fn write(&mut self, record: &Record) -> CliResult<()> {
        let qual = record.qual().ok_or("Qualities missing!")?;
        // Using .raw_seq() is possible only because FASTA cannot be used as input source
        // (no quality info). Might change if getting the quality info from other sources
        // (mothur-style .qual files)
        let seq = record.raw_seq();

        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => fastq::write_parts(&mut self.0, id, desc, seq, qual)?,
            SeqHeader::FullHeader(h) => fastq::write_to(&mut self.0, h, seq, qual)?,
        }
        Ok(())
    }
}
