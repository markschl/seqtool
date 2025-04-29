use std::fmt;
use std::io;
use std::path::Path;
use std::str::FromStr;

use itertools::Itertools;

use super::{QualFormat, DEFAULT_IO_READER_BUFSIZE, DEFAULT_IO_WRITER_BUFSIZE};

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
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
pub enum CompressionFormat {
    #[cfg(feature = "gz")]
    Gzip,
    #[cfg(feature = "bz2")]
    Bzip2,
    #[cfg(feature = "lz4")]
    Lz4,
    #[cfg(feature = "zstd")]
    Zstd,
}

impl CompressionFormat {
    const FORMAT_MAP: &[(&[&str], CompressionFormat)] = &[
        #[cfg(feature = "gz")]
        (&["gz", "gzip"], CompressionFormat::Gzip),
        #[cfg(feature = "bz2")]
        (&["bz2", "bzip2"], CompressionFormat::Bzip2),
        #[cfg(feature = "lz4")]
        (&["lz4"], CompressionFormat::Lz4),
        #[cfg(feature = "zstd")]
        (&["zst", "zstd", "zstandard"], CompressionFormat::Zstd),
    ];

    pub fn str_match(s: &str) -> Option<CompressionFormat> {
        let s = s.to_ascii_lowercase();
        for (names, format) in Self::FORMAT_MAP {
            if names.contains(&s.as_str()) {
                return Some(*format);
            }
        }
        None
    }

    pub fn recommended_read_bufsize(self) -> usize {
        match self {
            #[cfg(feature = "zstd")]
            CompressionFormat::Zstd => zstd::Decoder::<io::Empty>::recommended_output_size(),
            _ => DEFAULT_IO_READER_BUFSIZE,
        }
    }

    pub fn recommended_write_bufsize(self) -> usize {
        match self {
            #[cfg(feature = "zstd")]
            CompressionFormat::Zstd => zstd::Encoder::<io::Sink>::recommended_input_size(),
            _ => DEFAULT_IO_WRITER_BUFSIZE,
        }
    }
}

impl FromStr for CompressionFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(format) = CompressionFormat::str_match(s) {
            Ok(format)
        } else {
            let fmt_list = CompressionFormat::FORMAT_MAP
                .iter()
                .map(|(names, _)| names.join("/"))
                .join(", ");
            Err(format!(
                "Unknown compression format: {}. Valid formats are: {}.",
                s, fmt_list
            ))
        }
    }
}

/// Information on the sequence format and compression
/// which can be inferred from the file extensions
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct FileInfo {
    pub format: FormatVariant,
    pub compression: Option<CompressionFormat>,
}

impl FileInfo {
    pub fn new(format: FormatVariant, compression: Option<CompressionFormat>) -> Self {
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
            Some(ext) => match CompressionFormat::from_str(ext.to_str().unwrap_or("")) {
                Ok(c) => {
                    _path = _path.file_stem().unwrap().into();
                    Some(c)
                }
                Err(_) => None,
            },
            None => None,
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
            Some(CompressionFormat::from_str(comp_str)?)
        } else {
            None
        };
        Ok(FileInfo {
            format,
            compression,
        })
    }
}
