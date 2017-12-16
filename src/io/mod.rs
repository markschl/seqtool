use error::CliResult;

pub use self::record::*;

pub trait SeqReader {
    fn next(&mut self) -> Option<CliResult<&Record>>;
}

pub trait SeqWriter {
    fn write(&mut self, id: &[u8], desc: Option<&[u8]>, record: &Record) -> CliResult<()>;
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum Compression {
    GZIP,
    BZIP2,
    LZ4,
}

mod record;
pub mod fasta;
pub mod fastq;
pub mod csv;
pub mod input;
pub mod output;
