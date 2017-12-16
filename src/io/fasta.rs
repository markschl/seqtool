use std::io;

use error::CliResult;
use seq_io::fasta;
use super::*;

// Record

impl<'a> Record for fasta::RefRecord<'a> {
    fn id_bytes(&self) -> &[u8] {
        <fasta::RefRecord as fasta::Record>::id_bytes(self)
    }

    fn desc_bytes(&self) -> Option<&[u8]> {
        <fasta::RefRecord as fasta::Record>::desc_bytes(self)
    }

    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        <fasta::RefRecord as fasta::Record>::id_desc_bytes(self)
    }

    fn raw_seq(&self) -> &[u8] {
        <fasta::RefRecord as fasta::Record>::seq(self)
    }

    fn qual(&self) -> Option<&[u8]> {
        None
    }

    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Fasta(self.seq_lines())
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

impl<W: io::Write> SeqWriter for FastaWriter<W> {
    fn write(&mut self, id: &[u8], desc: Option<&[u8]>, record: &Record) -> CliResult<()> {
        if let Some(wrap) = self.wrap {
            fasta::write_id_desc(&mut self.io_writer, id, desc)?;
            fasta::write_wrap_seq_iter(&mut self.io_writer, record.seq_segments(), wrap)?;
        } else {
            fasta::write_id_desc(&mut self.io_writer, id, desc)?;
            fasta::write_seq_iter(&mut self.io_writer, record.seq_segments())?;
        }
        Ok(())
    }
}
