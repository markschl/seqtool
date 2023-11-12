use std::io;

use crate::error::CliResult;
use crate::var;

pub use self::qual_format::*;
pub use self::record::*;

pub trait SeqReader<O> {
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>>;
}

pub trait SeqWriter<W: io::Write> {
    fn write(&mut self, record: &dyn Record, vars: &var::Vars) -> CliResult<()>;
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>>;
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum Compression {
    None,
    Gzip,
    Bzip2,
    Lz4,
    Zstd,
}

impl Compression {
    pub fn from_str(s: &str) -> Option<Compression> {
        match s {
            "gz" => Some(Compression::Gzip),
            "bz2" => Some(Compression::Bzip2),
            "lz4" => Some(Compression::Lz4),
            "zst" => Some(Compression::Zstd),
            _ => None,
        }
    }

    pub fn best_read_bufsize(self) -> usize {
        match self {
            Compression::Zstd => zstd::Decoder::<io::Empty>::recommended_output_size(),
            _ => 1 << 22,
        }
    }

    pub fn best_write_bufsize(self) -> usize {
        match self {
            Compression::Zstd => zstd::Encoder::<io::Sink>::recommended_input_size(),
            _ => 1 << 22,
        }
    }
}

pub mod csv;
pub mod fa_qual;
pub mod fasta;
pub mod fastq;
pub mod input;
pub mod output;
mod qual_format;
mod record;
