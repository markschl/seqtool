use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::{convert::Infallible, path::Path};

use csv::DEFAULT_OUTFIELDS;
use fastx::Attribute;
use itertools::Itertools;
use thread_io;

use crate::error::CliResult;
use crate::var::VarBuilder;

use super::input::InFormat;
use super::{CompressionFormat, FormatVariant, QualFormat, Record, DEFAULT_IO_WRITER_BUFSIZE};

pub use self::writer::*;

pub mod csv;
pub mod fa_qual;
pub mod fasta;
pub mod fastq;
pub mod fastx;
pub mod writer;

/// Format options for creating output streams
#[derive(Clone, Debug)]
pub struct SeqWriterOpts {
    /// output file format
    pub format: Option<FormatVariant>,
    /// FASTX head attributes
    pub attrs: Vec<(Attribute, bool)>,
    pub wrap_fasta: Option<usize>,
    /// Configured text delimiter (overrides choices by FormatVariant::Tsv and FormatVariant::Csv)
    pub delim: Option<char>,
    /// Delimited text fields (if known from args)
    pub fields: Option<String>,
    // .qual file path
    pub qfile: Option<String>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct OutputOpts {
    /// append to files?
    pub append: bool,
    pub compression_format: Option<CompressionFormat>,
    pub compression_level: Option<u8>,
    pub threaded: bool,
    pub thread_bufsize: Option<usize>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum OutputKind {
    Stdout,
    File(String),
}

impl FromStr for OutputKind {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(OutputKind::Stdout)
        } else {
            Ok(OutputKind::File(s.to_string()))
        }
    }
}

impl OutputKind {
    // pub fn get_info(&self, default_format: FormatVariant) -> FileInfo {
    //     match self {
    //         OutputKind::Stdout => FileInfo::new(default_format, None),
    //         OutputKind::File(path) => FileInfo::from_path(path, default_format, true),
    //     }
    // }

    /// Returns an I/O writer.
    /// Ignores `threaded` and `thread_bufsize` options.
    /// The caller is responsible for calling `finish()` on the writer when done.
    pub fn get_io_writer(&self, opts: &OutputOpts) -> io::Result<Box<dyn WriteFinish>> {
        let writer: Box<dyn WriteFinish> = match self {
            OutputKind::Stdout => Box::new(io::BufWriter::new(io::stdout().lock())),
            OutputKind::File(ref p) => {
                let f = File::options()
                    .create(true)
                    .write(true)
                    .truncate(!opts.append)
                    .append(opts.append)
                    .open(p)
                    .map_err(|e| io::Error::other(format!("Error creating '{}': {}", p, e)))?;
                Box::new(io::BufWriter::new(f))
            }
        };
        if let Some(fmt) = opts.compression_format {
            return compr_writer(writer, fmt, opts.compression_level);
        }
        Ok(writer)
    }

    /// Creates an I/O writer either in the main thread (no compression)
    /// or in a background thread (if explicitly specified or writing to
    /// compressed format).
    pub fn with_io_writer<F, O>(&self, opts: &OutputOpts, func: F) -> CliResult<O>
    where
        F: FnOnce(&mut dyn io::Write) -> CliResult<O>,
    {
        if opts.compression_format.is_some() || opts.threaded {
            let thread_bufsize = opts.thread_bufsize.unwrap_or_else(|| {
                opts.compression_format
                    .map(|c| c.recommended_write_bufsize())
                    .unwrap_or(DEFAULT_IO_WRITER_BUFSIZE)
            });

            thread_io::write::writer_init_finish(
                thread_bufsize,
                4,
                || {
                    let writer = self.get_io_writer(opts)?;
                    Ok(writer)
                },
                |mut w| func(&mut w),
                |w| w.finish().map(|_| ()),
            )
            .map(|(o, _)| o)
        } else {
            let mut writer = self.get_io_writer(opts)?;
            let o = func(&mut writer)?;
            writer.finish()?;
            Ok(o)
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum OutFormat {
    Fasta {
        /// (attribute, replace_existing)
        attrs: Vec<(Attribute, bool)>,
        // Vec<(attr_name, attr_value)>, default_seqattr_for_attrs
        wrap_width: Option<usize>,
    },
    Fastq {
        format: QualFormat,
        /// (attribute, replace_existing)
        attrs: Vec<(Attribute, bool)>,
    },
    FaQual {
        /// (attribute, replace_existing)
        attrs: Vec<(Attribute, bool)>,
        wrap_width: Option<usize>,
        qfile: PathBuf,
    },
    DelimitedText {
        delim: u8,
        // this field list is not in Vec<String> form because parsing
        // output fields is more complex (functions can have have commas inside)
        fields: String,
    },
}

impl OutFormat {
    pub fn default_ext(&self) -> &'static str {
        match *self {
            OutFormat::Fasta { .. } => "fasta",
            OutFormat::Fastq { .. } => "fastq",
            OutFormat::FaQual { .. } => "fasta",
            OutFormat::DelimitedText { delim, .. } => {
                if delim == b'\t' {
                    "tsv"
                } else {
                    "csv"
                }
            }
        }
    }

    pub fn from_opts(opts: &SeqWriterOpts, in_format: &InFormat) -> CliResult<OutFormat> {
        let format = opts.format.unwrap_or_else(|| in_format.format_variant());
        let mut format = match format {
            FormatVariant::Fasta => OutFormat::Fasta {
                attrs: opts.attrs.to_owned(),
                wrap_width: opts.wrap_fasta,
            },
            FormatVariant::Fastq(qformat) => OutFormat::Fastq {
                format: qformat,
                attrs: opts.attrs.to_owned(),
            },
            FormatVariant::Csv => get_delim_format(opts.fields.clone(), opts.delim, in_format, ','),
            FormatVariant::Tsv => {
                get_delim_format(opts.fields.clone(), opts.delim, in_format, '\t')
            }
        };

        // FaQual format
        if let Some(f) = opts.qfile.as_ref() {
            match format {
                OutFormat::Fasta { attrs, wrap_width } => {
                    format = OutFormat::FaQual {
                        attrs,
                        wrap_width,
                        qfile: PathBuf::from(f),
                    };
                }
                _ => return fail!("Expecting FASTA as output format if combined with QUAL files"),
            }
        }
        Ok(format)
    }

    pub fn get_writer<'a>(
        &self,
        builder: &mut VarBuilder,
    ) -> CliResult<Box<dyn FormatWriter + 'a>> {
        Ok(match self {
            OutFormat::Fasta {
                ref attrs,
                wrap_width,
            } => Box::new(fasta::FastaWriter::new(*wrap_width, attrs, builder)?),
            OutFormat::Fastq { format, ref attrs } => {
                Box::new(fastq::FastqWriter::new(*format, attrs, builder)?)
            }
            OutFormat::FaQual {
                ref attrs,
                wrap_width,
                ref qfile,
            } => Box::new(fa_qual::FaQualWriter::new(
                *wrap_width,
                qfile,
                attrs,
                builder,
            )?),
            OutFormat::DelimitedText { delim, ref fields } => {
                Box::new(csv::CsvWriter::new(fields, *delim, builder)?)
            }
        })
    }
}

/// Derives text field list and delimiter from input format if not set
/// and returns OutFormat::DelimitedText {...}
#[cold]
pub fn get_delim_format(
    mut fields: Option<String>,
    mut delim: Option<char>,
    informat: &InFormat,
    fallback_delimiter: char,
) -> OutFormat {
    if fields.is_none() || delim.is_none() {
        if let InFormat::DelimitedText {
            delim: d,
            fields: ref f,
            ..
        } = informat
        {
            if fields.is_none() {
                fields = Some(f.iter().map(|(f, _)| f).join(","));
            }
            if delim.is_none() {
                delim = Some(*d as char);
            }
        }
    }
    OutFormat::DelimitedText {
        delim: delim.unwrap_or(fallback_delimiter) as u8,
        fields: fields.unwrap_or_else(|| DEFAULT_OUTFIELDS.to_string()),
    }
}

/// Helper trait to finish compression streams in an unified way.
/// All writers are additionally flushed.
pub trait WriteFinish: io::Write {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a;
}

impl<W: io::Write> WriteFinish for io::BufWriter<W> {
    fn finish<'a>(mut self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        self.flush()?;
        Ok(self)
    }
}

#[cfg(feature = "lz4")]
impl<W: io::Write> WriteFinish for lz4::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        let (mut w, res) = (*self).finish();
        w.flush()?;
        res.map(|_| Box::new(w) as Box<dyn io::Write>)
    }
}

#[cfg(feature = "zstd")]
impl<W: io::Write> WriteFinish for zstd::Encoder<'_, W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().and_then(|mut w| {
            w.flush()?;
            Ok(Box::new(w) as Box<dyn io::Write>)
        })
    }
}

#[cfg(feature = "gz")]
impl<W: io::Write> WriteFinish for flate2::write::GzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().and_then(|mut w| {
            w.flush()?;
            Ok(Box::new(w) as Box<dyn io::Write>)
        })
    }
}

#[cfg(feature = "bz2")]
impl<W: io::Write> WriteFinish for bzip2::write::BzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().and_then(|mut w| {
            w.flush()?;
            Ok(Box::new(w) as Box<dyn io::Write>)
        })
    }
}

/// Returns a general I/O writer (not for sequence writing), given a path
/// (or '-' for STDOUT), automatically recognizing possible compression
/// from the extension if `opts.compression_format` is not set.
/// Ignores `threaded` and `thread_bufsize` options.
/// The caller is responsible for calling `finish()` on the writer when done.
pub fn io_writer_from_path<P>(path: P, mut opts: OutputOpts) -> CliResult<Box<dyn WriteFinish>>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let path_str = path
        .to_str()
        .ok_or_else(|| format!("Invalid path: '{}'", path.to_string_lossy()))?;
    let kind = OutputKind::from_str(path_str).unwrap();
    if opts.compression_format.is_none() {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        opts.compression_format = CompressionFormat::str_match(ext);
    }
    let out = kind.get_io_writer(&opts)?;
    Ok(out)
}

// /// Provides a scoped general I/O writer (not for sequence writing), taking
// /// care of cleanup when done.
// /// See also `general_io_writer`.
// pub fn with_general_io_writer<P, F>(path: P, func: F) -> CliResult<()>
// where
//     P: AsRef<Path>,
//     F: FnOnce(&mut dyn io::Write) -> CliResult<()>,
// {
//     let mut compr_writer = general_io_writer(path)?;
//     func(&mut compr_writer)?;
//     compr_writer.finish()?;
//     Ok(())
// }

fn compr_writer(
    writer: Box<dyn WriteFinish>,
    compr_format: CompressionFormat,
    compr_level: Option<u8>,
) -> io::Result<Box<dyn WriteFinish>> {
    Ok(match compr_format {
        #[cfg(feature = "gz")]
        CompressionFormat::Gzip => Box::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::new(u32::from(compr_level.unwrap_or(6))),
        )),
        #[cfg(feature = "bz2")]
        CompressionFormat::Bzip2 => {
            let c = match compr_level {
                Some(l) => bzip2::Compression::new(l as u32),
                _ => bzip2::Compression::default(),
            };
            Box::new(bzip2::write::BzEncoder::new(writer, c))
        }
        #[cfg(feature = "lz4")]
        CompressionFormat::Lz4 => Box::new(
            lz4::EncoderBuilder::new()
                .level(compr_level.unwrap_or(0) as u32)
                .build(writer)?,
        ),
        #[cfg(feature = "zstd")]
        CompressionFormat::Zstd => Box::new(zstd::Encoder::new(
            writer,
            i32::from(compr_level.unwrap_or(0)),
        )?),
    })
}
