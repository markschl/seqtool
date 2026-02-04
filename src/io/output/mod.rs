use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use csv::DEFAULT_OUTFIELDS;
use fastx::Attribute;
use itertools::Itertools;
use thread_io;

use crate::error::CliResult;
use crate::var::VarBuilder;

use super::input::InFormat;
use super::{
    CompressionFormat, DEFAULT_FORMAT, DEFAULT_IO_WRITER_BUFSIZE, FormatVariant, IoKind,
    QualFormat, Record, parse_compr_ext,
};

pub use self::writer::*;

pub mod csv;
pub mod fa_qual;
pub mod fasta;
pub mod fastq;
pub mod fastx;
pub mod writer;

/// Combined configuration of an output stream
#[derive(Debug)]
pub struct OutputConfig {
    pub kind: Option<IoKind>,
    pub writer: OutputOpts,
    pub format: OutFormat,
}

/// General output options
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct OutputOpts {
    /// append to files?
    pub append: bool,
    pub compression_format: Option<CompressionFormat>,
    pub compression_level: Option<u8>,
    pub threaded: bool,
    pub thread_bufsize: Option<usize>,
}

/// Formatting options
#[derive(Clone, Debug)]
pub struct FormatOpts {
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

impl IoKind {
    /// Returns an I/O writer
    pub fn simple_io_writer(&self, append: bool) -> io::Result<Box<dyn WriteFinish>> {
        match self {
            IoKind::Stdio => Ok(Box::new(io::BufWriter::new(io::stdout().lock()))),
            IoKind::File(p) => {
                let f = File::options()
                    .create(true)
                    .write(true)
                    .truncate(!append)
                    .append(append)
                    .open(p)
                    .map_err(|e| {
                        io::Error::other(format!("Error creating '{}': {}", p.to_string_lossy(), e))
                    })?;
                Ok(Box::new(io::BufWriter::new(f)))
            }
        }
    }

    /// Returns an I/O writer given an output options object.
    /// Ignores `threaded` and `thread_bufsize` options.
    /// The caller is responsible for calling `finish()` on the writer when done.
    pub fn io_writer(&self, opts: &OutputOpts) -> io::Result<Box<dyn WriteFinish>> {
        let writer: Box<dyn WriteFinish> = self.simple_io_writer(opts.append)?;
        if let Some(fmt) = opts.compression_format {
            return compr_writer(writer, fmt, opts.compression_level);
        }
        Ok(writer)
    }

    /// Creates an I/O writer either in the main thread (no compression)
    /// or in a background thread (if explicitly specified or writing to
    /// compressed format).
    pub fn with_thread_writer<F, O>(&self, opts: &OutputOpts, func: F) -> CliResult<O>
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
                    let writer = self.io_writer(opts)?;
                    Ok(writer)
                },
                |mut w| func(&mut w),
                |w| w.finish().map(|_| ()),
            )
            .map(|(o, _)| o)
        } else {
            let mut writer = self.io_writer(opts)?;
            let o = func(&mut writer)?;
            writer.finish()?;
            Ok(o)
        }
    }
}

/// Infers the ouput format compression and sequence format
/// (1) from the path extension
/// (2) from the input format
/// `format_opts.format` is defined after this call
pub fn infer_out_format(
    out_kind: Option<&IoKind>,
    in_format: &InFormat,
    out_opts: &mut OutputOpts,
    format_opts: &mut FormatOpts,
) {
    if (out_opts.compression_format.is_none() || format_opts.format.is_none())
        && let Some(IoKind::File(path)) = out_kind
    {
        let (compression, ext) = parse_compr_ext(&path);
        if out_opts.compression_format.is_none() {
            out_opts.compression_format = compression;
        }
        if format_opts.format.is_none() {
            format_opts.format = ext.and_then(FormatVariant::str_match);
            if format_opts.format.is_none() && format_opts.qfile.is_none() {
                eprintln!(
                    "Could not infer the output format from the extension of '{}', \
                        defaulting to the input format",
                    path.to_string_lossy()
                );
            }
        }
    }
    if format_opts.format.is_none()
        || matches!(
            format_opts.format,
            Some(FormatVariant::Csv) | Some(FormatVariant::Tsv)
        )
    {
        let (fmt, fields, delim) = in_format.components();
        if format_opts.format.is_none() {
            format_opts.format = Some(fmt);
            // only set the delimiter if the format was previously not set
            // (since the delimiter is tied to FormatVariant)
            if format_opts.delim.is_none() {
                format_opts.delim = delim;
            }
        }
        // infer the fields from the input in any case
        if format_opts.fields.is_none() {
            format_opts.fields = fields.map(|f| f.iter().map(|(n, _)| n).join(","));
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

    pub fn from_opts(opts: &FormatOpts) -> CliResult<OutFormat> {
        // FaQual format: we ignore opts.format
        // as some validation has been done in CommonArgs::get_output_opts()
        if let Some(f) = opts.qfile.as_ref() {
            return Ok(OutFormat::FaQual {
                attrs: opts.attrs.to_owned(),
                wrap_width: opts.wrap_fasta,
                qfile: PathBuf::from(f),
            });
        }

        // we assume that opts.format is defined at this point
        debug_assert!(opts.format.is_some());
        let format = match opts.format.unwrap_or(DEFAULT_FORMAT) {
            FormatVariant::Fasta => OutFormat::Fasta {
                attrs: opts.attrs.to_owned(),
                wrap_width: opts.wrap_fasta,
            },
            FormatVariant::Fastq(qformat) => OutFormat::Fastq {
                format: qformat,
                attrs: opts.attrs.to_owned(),
            },
            f @ (FormatVariant::Csv | FormatVariant::Tsv) => OutFormat::DelimitedText {
                delim: opts
                    .delim
                    .unwrap_or(if f == FormatVariant::Csv { ',' } else { '\t' })
                    as u8,
                fields: opts
                    .fields
                    .clone()
                    .unwrap_or_else(|| DEFAULT_OUTFIELDS.to_string()),
            },
        };
        Ok(format)
    }

    pub fn get_formatter<'a>(
        &self,
        builder: &mut VarBuilder,
    ) -> CliResult<Box<dyn SeqFormatter + 'a>> {
        Ok(match self {
            OutFormat::Fasta { attrs, wrap_width } => {
                Box::new(fasta::FastaWriter::new(*wrap_width, attrs, builder)?)
            }
            OutFormat::Fastq { format, attrs } => {
                Box::new(fastq::FastqWriter::new(*format, attrs, builder)?)
            }
            OutFormat::FaQual {
                attrs,
                wrap_width,
                qfile,
            } => Box::new(fa_qual::FaQualWriter::new(
                *wrap_width,
                qfile,
                attrs,
                builder,
            )?),
            OutFormat::DelimitedText { delim, fields } => {
                Box::new(csv::CsvWriter::new(fields, *delim, builder)?)
            }
        })
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
