use std::cell::Cell;

use memchr::memchr;
use seq_io::fastq::{self, Reader, Record as FR};
use seq_io::policy::BufPolicy;

use super::*;
use crate::error::CliResult;
use crate::var;

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
    delim: Cell<Option<Option<usize>>>,
}

impl<'a> FastqRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fastq::RefRecord<'a>) -> FastqRecord<'a> {
        FastqRecord {
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

pub struct FastqWriter {
    qual_fmt: Option<QualFormat>,
    qual_vec: Vec<u8>,
}

impl FastqWriter {
    pub fn new(qual_fmt: Option<QualFormat>) -> Self {
        Self {
            qual_fmt,
            qual_vec: vec![],
        }
    }
}

impl SeqWriter for FastqWriter {
    fn write<W: io::Write>(&mut self, record: &dyn Record, vars: &var::Vars, mut out: W) -> CliResult<()> {
        let qual = record.qual().ok_or("No quality scores found in input.")?;
        let qual = if let Some(fmt) = self.qual_fmt {
            self.qual_vec.clear();
            vars.data()
                .qual_converter
                .convert_quals(qual, &mut self.qual_vec, fmt)
                .map_err(|e| {
                    format!(
                        "Error writing record '{}'. {}",
                        String::from_utf8_lossy(record.id_bytes()),
                        e
                    )
                })?;
            &self.qual_vec
        } else {
            qual
        };

        // TODO: could use seq_io::fastq::write_to / write_parts, but the sequence is an iterator of segments

        out.write_all(b"@")?;

        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => {
                out.write_all(id)?;
                if let Some(d) = desc {
                    out.write_all(b" ")?;
                    out.write_all(d)?;
                }
            }
            SeqHeader::FullHeader(h) => {
                out.write_all(h)?;
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
