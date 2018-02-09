use std::io;
use zstd;
use error::CliResult;

pub use self::record::*;


pub trait SeqReader {
    fn next(&mut self) -> Option<CliResult<&Record>>;
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
pub mod fasta;
pub mod fastq;
pub mod csv;
pub mod input;
pub mod output;
