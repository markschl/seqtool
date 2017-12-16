use std::io;
use error::CliResult;

use seq_io::fastq;
use super::*;

// Record

impl<'a> Record for fastq::RefRecord<'a> {
    fn id_bytes(&self) -> &[u8] {
        <fastq::RefRecord as fastq::Record>::id_bytes(self)
    }

    fn desc_bytes(&self) -> Option<&[u8]> {
        <fastq::RefRecord as fastq::Record>::desc_bytes(self)
    }

    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        <fastq::RefRecord as fastq::Record>::id_desc_bytes(self)
    }

    fn raw_seq(&self) -> &[u8] {
        <fastq::RefRecord as fastq::Record>::seq(self)
    }

    fn qual(&self) -> Option<&[u8]> {
        Some(<fastq::RefRecord as fastq::Record>::qual(self))
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        to.extend_from_slice(self.raw_seq())
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
    fn write(&mut self, id: &[u8], desc: Option<&[u8]>, record: &Record) -> CliResult<()> {
        let qual = record.qual().ok_or("Qualities missing!")?;
        // Using .raw_seq() is possible only because FASTA cannot be used as input source
        // (no quality info). Might become a problem if getting the quality info from other sources
        // (mothur-style .qual files)
        let seq = record.raw_seq();
        fastq::write_parts(&mut self.0, id, desc, seq, qual)?;
        Ok(())
    }
}
