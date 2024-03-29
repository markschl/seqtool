use std::fmt;
use std::io;
use std::path::Path;
use std::str::FromStr;

use crate::error::CliResult;

pub use self::qual_format::*;
pub use self::record::*;

pub mod csv;
pub mod fa_qual;
pub mod fasta;
pub mod fastq;
mod fastx;
pub mod input;
pub mod output;
mod qual_format;
mod record;

pub trait SeqReader<O> {
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>>;
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum FormatVariant {
    Fasta,
    Fastq(QualFormat),
    Csv,
    Tsv,
}

impl fmt::Display for FormatVariant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FormatVariant::Fasta => write!(f, "fasta"),
            FormatVariant::Fastq(fmt) => match fmt {
                QualFormat::Sanger | QualFormat::Phred => write!(f, "fastq"),
                QualFormat::Illumina => write!(f, "fastq-illumina"),
                QualFormat::Solexa => write!(f, "fastq-solexa"),
            },
            FormatVariant::Csv => write!(f, "csv"),
            FormatVariant::Tsv => write!(f, "tsv"),
        }
    }
}

impl FromStr for FormatVariant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "fasta" | "fa" | "fna" => Ok(FormatVariant::Fasta),
            "fastq" | "fq" => Ok(FormatVariant::Fastq(QualFormat::Sanger)),
            "fastq-illumina" | "fq-illumina" => Ok(FormatVariant::Fastq(QualFormat::Illumina)),
            "fastq-solexa" | "fq-solexa" => Ok(FormatVariant::Fastq(QualFormat::Solexa)),
            "csv" => Ok(FormatVariant::Csv),
            "tsv" | "txt" => Ok(FormatVariant::Tsv),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum Compression {
    None,
    #[cfg(feature = "gz")]
    Gzip,
    #[cfg(feature = "bz2")]
    Bzip2,
    #[cfg(feature = "lz4")]
    Lz4,
    #[cfg(feature = "zstd")]
    Zstd,
}

impl Compression {
    pub fn best_read_bufsize(self) -> usize {
        match self {
            #[cfg(feature = "zstd")]
            Compression::Zstd => zstd::Decoder::<io::Empty>::recommended_output_size(),
            _ => 1 << 22,
        }
    }

    pub fn best_write_bufsize(self) -> usize {
        match self {
            #[cfg(feature = "zstd")]
            Compression::Zstd => zstd::Encoder::<io::Sink>::recommended_input_size(),
            _ => 1 << 22,
        }
    }
}

impl FromStr for Compression {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            #[cfg(feature = "gz")]
            "gz" | "gzip" => Ok(Compression::Gzip),
            #[cfg(feature = "bz2")]
            "bz2" | "bzip2" => Ok(Compression::Bzip2),
            #[cfg(feature = "lz4")]
            "lz4" => Ok(Compression::Lz4),
            #[cfg(feature = "zstd")]
            "zst" | "zstd" | "zstandard" => Ok(Compression::Zstd),
            _ => Err(format!("Unknown compression format: {}. Valid formats are gz (gzip), bz2 (bzip2), lz4 and zst (zstd, zstandard).", s)),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct FileInfo {
    pub format: FormatVariant,
    pub compression: Compression,
}

impl FileInfo {
    pub fn new(format: FormatVariant, compression: Compression) -> Self {
        Self {
            format,
            compression,
        }
    }

    pub fn from_path<P: AsRef<Path>>(
        path: P,
        default_format: FormatVariant,
        report_default: bool,
    ) -> Self {
        let mut _path = path.as_ref().to_owned();

        let compression = match _path.extension() {
            Some(ext) => match Compression::from_str(ext.to_str().unwrap_or("")) {
                Ok(c) => {
                    _path = _path.file_stem().unwrap().into();
                    c
                }
                Err(_) => Compression::None,
            },
            None => Compression::None,
        };

        let format = match _path.extension() {
            Some(ext) => match FormatVariant::from_str(ext.to_str().unwrap_or("")) {
                Ok(f) => f,
                Err(_) => {
                    let ext = ext.to_string_lossy();
                    if ext.find('{').is_none() {
                        // print message unless extension is a variable/function
                        eprintln!(
                            "Unknown extension: '{}', assuming {} format",
                            ext, default_format
                        );
                    }
                    default_format
                }
            },
            None => {
                if report_default {
                    eprintln!(
                        "No extension for file '{}' assuming {} format",
                        path.as_ref().to_string_lossy(),
                        default_format
                    );
                }
                default_format
            }
        };

        Self {
            format,
            compression,
        }
    }
}

impl FromStr for FileInfo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '.');
        let format = FormatVariant::from_str(parts.next().unwrap())?;
        let compression = if let Some(comp_str) = parts.next() {
            Compression::from_str(comp_str)?
        } else {
            Compression::None
        };
        Ok(FileInfo {
            format,
            compression,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

impl FromStr for Attribute {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '=');
        let name = parts.next().unwrap().to_string();
        let value = match parts.next() {
            Some(p) => p.to_string(),
            None => {
                return Err(format!(
                    "Invalid attribute: '{}'. Attributes need to be in the format: name=value",
                    name
                ))
            }
        };
        Ok(Attribute { name, value })
    }
}
