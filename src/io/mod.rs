use std::io;
use zstd;
use var;
use error::CliResult;

pub use self::record::*;
pub use self::qual_format::*;


pub trait SeqReader<O> {
    fn read_next(&mut self, func: &mut FnMut(&Record) -> O) -> Option<CliResult<O>>;
}


pub trait SeqWriter<W: io::Write> {
    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()>;
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>>;
}



#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum Compression {
    None,
    GZIP,
    BZIP2,
    LZ4,
    ZSTD,
}

impl Compression {
    pub fn from_str(s: &str) -> Option<Compression> {
        match s {
            "gz" => Some(Compression::GZIP),
            "bz2" => Some(Compression::BZIP2),
            "lz4" => Some(Compression::LZ4),
            "zst" => Some(Compression::ZSTD),
            _ => None
        }
    }

    pub fn best_read_bufsize(&self) -> usize {
        match *self {
            Compression::ZSTD => zstd::Decoder::<io::Empty>::recommended_output_size(),
            _ => 1 << 22
        }
    }

    pub fn best_write_bufsize(&self) -> usize {
        match *self {
            Compression::ZSTD => zstd::Encoder::<io::Sink>::recommended_input_size(),
            _ => 1 << 22
        }
    }
}


mod record;
mod qual_format;
pub mod fasta;
pub mod fastq;
pub mod csv;
pub mod input;
pub mod output;
