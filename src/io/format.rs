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

impl FormatVariant {
    pub fn str_match(s: &str) -> Option<FormatVariant> {
        match s.to_ascii_lowercase().as_str() {
            "fasta" | "fa" | "fna" => Some(FormatVariant::Fasta),
            "fastq" | "fq" => Some(FormatVariant::Fastq(QualFormat::Sanger)),
            "fastq-illumina" | "fq-illumina" => Some(FormatVariant::Fastq(QualFormat::Illumina)),
            "fastq-solexa" | "fq-solexa" => Some(FormatVariant::Fastq(QualFormat::Solexa)),
            "csv" => Some(FormatVariant::Csv),
            "tsv" | "txt" => Some(FormatVariant::Tsv),
            _ => None,
        }
    }
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
        FormatVariant::str_match(s).ok_or_else(|| format!("Unknown format: {}", s))
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

/// Parses a single or double extension from a path:
/// If the extension is recognized as a compression format,
/// it is returned along with the inner extension.
/// Otherwise, only the outer extension is returned (no compression assumed).
pub fn parse_compr_ext<P: AsRef<Path> + ?Sized>(
    path: &P,
) -> (Option<CompressionFormat>, Option<&str>) {
    let path = path.as_ref();
    let mut fmt = None;
    let mut ext = None;
    if let Some(e) = path.extension().and_then(|e| e.to_str()) {
        if let Some(f) = CompressionFormat::str_match(e) {
            fmt = Some(f);
            if let Some(e) = Path::new(path.file_stem().unwrap()).extension() {
                ext = e.to_str();
            }
        } else {
            ext = Some(e);
        }
    }
    (fmt, ext)
}
