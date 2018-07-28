
use std::io::Write;
use error::CliResult;
use std::borrow::Cow;
use std::cell::Cell;

use memchr::memchr;

use seq_io::BufStrategy;
use seq_io::fastq::{self, Record as FR, Reader};
use var;
use super::*;


// Reader

pub struct FastqReader<R: io::Read, S: BufStrategy>(pub Reader<R, S>);

impl<R, S, O> SeqReader<O> for FastqReader<R, S>
    where
        R: io::Read,
        S: BufStrategy,
{
    fn read_next(&mut self, func: &mut FnMut(&Record) -> O) -> Option<CliResult<O>> {
        self.0.next().map(|r| {
            let r = FastqRecord::new(r?);
            Ok(func(&r))
        })
    }
}


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

pub struct FastqWriter<W: io::Write> {
    writer: W,
    qual_fmt: Option<QualFormat>,
    qual_vec: Vec<u8>,
}

impl<W: io::Write> FastqWriter<W> {
    pub fn new(io_writer: W, qual_fmt: Option<QualFormat>) -> FastqWriter<W> {

        FastqWriter {
            writer: io_writer,
            qual_fmt: qual_fmt,
            qual_vec: vec![],
        }
    }
}


impl<W: io::Write> SeqWriter<W> for FastqWriter<W> {
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        let qual = record.qual().ok_or("No quality scores found in input.")?;
        let qual =
            if let Some(fmt) = self.qual_fmt {
                self.qual_vec.clear();
                vars.data().qual_converter
                    .convert_quals(qual, &mut self.qual_vec, fmt)
                    .map_err(|e| format!(
                        "Error writing record '{}'. {}",
                        String::from_utf8_lossy(record.id_bytes()), e
                    ))?;
                &self.qual_vec
            } else { qual };

        // Using .raw_seq() is possible only because FASTA cannot be used as input source
        // (no quality info). Might change if getting the quality info from other sources
        // (mothur-style .qual files)
        let seq = record.raw_seq();

        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => fastq::write_parts(&mut self.writer, id, desc, seq, qual)?,
            SeqHeader::FullHeader(h) => fastq::write_to(&mut self.writer, h, seq, qual)?,
        }
        Ok(())
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Some(Ok(self.writer))
    }
}
