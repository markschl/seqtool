use std::borrow::{Cow, ToOwned};
use std::cell::Cell;
use std::cmp::min;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use itertools::Itertools;
use memchr::memchr;

use error::CliResult;
use seq_io::fasta::{self, Record as FR};
use seq_io::BufStrategy;
use var;

use super::*;

// Reader

pub struct FaQualReader<R: io::Read, S: BufStrategy> {
    fa_rdr: fasta::Reader<R, S>,
    qual_rdr: fasta::Reader<File, S>,
    quals: Vec<u8>,
}

impl<R, S> FaQualReader<R, S>
where
    R: io::Read,
    S: BufStrategy + Clone,
{
    pub fn new<Q>(rdr: R, cap: usize, strategy: S, qfile: Q) -> CliResult<Self>
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
            fa_rdr: fasta::Reader::with_cap_and_strategy(rdr, cap, strategy.clone()),
            qual_rdr: fasta::Reader::with_cap_and_strategy(qhandle, cap, strategy),
            quals: vec![],
        })
    }
}

impl<R, S, O> SeqReader<O> for FaQualReader<R, S>
where
    R: io::Read,
    S: BufStrategy,
{
    fn read_next(&mut self, func: &mut FnMut(&Record) -> O) -> Option<CliResult<O>> {
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
        if b < b'0' || b > b'9' {
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
    fn delim(&self) -> Option<Option<usize>> {
        self.fa_rec.delim()
    }
    fn set_delim(&self, delim: Option<usize>) {
        self.fa_rec.set_delim(delim)
    }
    fn get_header(&self) -> SeqHeader {
        self.fa_rec.get_header()
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

pub struct FaQualWriter<W: io::Write> {
    fa_writer: super::fasta::FastaWriter<W>,
    qual_writer: io::BufWriter<File>,
    wrap: usize,
}

impl<W: io::Write> FaQualWriter<W> {
    pub fn new<Q>(fa_writer: W, wrap: Option<usize>, qual_path: Q) -> CliResult<FaQualWriter<W>>
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
            fa_writer: super::fasta::FastaWriter::new(fa_writer, wrap),
            qual_writer: io::BufWriter::new(q_handle),
            wrap: wrap.unwrap_or(::std::usize::MAX),
        })
    }
}

impl<W: io::Write> SeqWriter<W> for FaQualWriter<W> {
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        self.fa_writer.write(record, vars)?;

        // write quality scores
        let qual = record.qual().ok_or("No quality scores found in input.")?;

        // header
        match record.get_header() {
            SeqHeader::IdDesc(id, desc) => fasta::write_id_desc(&mut self.qual_writer, id, desc)?,
            SeqHeader::FullHeader(h) => fasta::write_head(&mut self.qual_writer, h)?,
        }

        // quality lines
        for qline in qual.chunks(self.wrap) {
            if !qline.is_empty() {
                let mut q_iter = qline.into_iter().map(|&q| {
                    vars.data()
                        .qual_converter
                        .convert(q, QualFormat::Phred)
                        .map_err(|e| {
                            format!(
                                "Error writing record '{}'. {}",
                                String::from_utf8_lossy(record.id_bytes()),
                                e
                            )
                        })
                });

                for q in q_iter.by_ref().take(qline.len() - 1) {
                    write!(self.qual_writer, "{} ", q?)?;
                }
                write!(self.qual_writer, "{}\n", q_iter.next().unwrap()?)?;
            }
        }

        Ok(())
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Box::new(self.fa_writer).into_inner()
    }
}
