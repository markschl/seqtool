use std::convert::Infallible;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use thread_io;

use crate::{error::CliResult, var::Vars};

use super::{fa_qual, fasta, fastq, Attribute, Compression, FormatVariant, QualFormat, Record};

pub use self::writer::*;

pub mod attr;
pub mod csv;
pub mod writer;

lazy_static! {
    static ref STDOUT: io::Stdout = io::stdout();
}

#[derive(Clone, Debug)]
pub struct OutputOptions {
    pub kind: OutputKind,
    pub format: OutFormat,
    pub compression: Compression,
    pub compression_level: Option<u8>,
    pub threaded: bool,
    pub thread_bufsize: Option<usize>,
}

impl OutputOptions {
    pub fn new(kind: OutputKind, format: OutFormat, compression: Compression) -> Self {
        Self {
            kind,
            format,
            compression,
            compression_level: None,
            threaded: false,
            thread_bufsize: None,
        }
    }

    pub fn thread_opts(mut self, threaded: bool, thread_bufsize: Option<usize>) -> Self {
        self.threaded = threaded;
        self.thread_bufsize = thread_bufsize;
        self
    }
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

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum OutFormat {
    Fasta {
        attrs: Vec<Attribute>,
        // Vec<(attr_name, attr_value)>, default_seqattr_for_attrs
        wrap_width: Option<usize>,
    },
    Fastq {
        format: QualFormat,
        attrs: Vec<Attribute>,
    },
    FaQual {
        attrs: Vec<Attribute>,
        wrap_width: Option<usize>,
        qfile: PathBuf,
    },
    Csv {
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
            OutFormat::Csv { delim, .. } => {
                if delim == b'\t' {
                    "tsv"
                } else {
                    "csv"
                }
            }
        }
    }

    pub fn from_opts(
        format: FormatVariant,
        attrs: &[Attribute],
        wrap_fasta: Option<usize>,
        csv_delim: Option<char>,
        csv_fields: &str,
        qfile: Option<&str>,
    ) -> CliResult<OutFormat> {
        let mut format = match format {
            FormatVariant::Fasta => OutFormat::Fasta {
                attrs: attrs.to_owned(),
                wrap_width: wrap_fasta,
            },
            FormatVariant::Fastq(qformat) => OutFormat::Fastq {
                format: qformat,
                attrs: attrs.to_owned(),
            },
            FormatVariant::Csv => OutFormat::Csv {
                delim: csv_delim.unwrap_or(',') as u8,
                fields: csv_fields.to_owned(),
            },
            FormatVariant::Tsv => OutFormat::Csv {
                delim: csv_delim.unwrap_or('\t') as u8,
                fields: csv_fields.to_owned(),
            },
        };

        // FaQual format
        if let Some(f) = qfile {
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
}

/// Helper trait to finish compression streams in an unified way.
pub trait WriteFinish: io::Write {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a;
}

impl<W: io::Write> WriteFinish for io::BufWriter<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        Ok(self)
    }
}

#[cfg(feature = "lz4")]
impl<W: io::Write> WriteFinish for lz4::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        let (w, res) = (*self).finish();
        res.map(|_| Box::new(w) as Box<dyn io::Write>)
    }
}

#[cfg(feature = "zstd")]
impl<W: io::Write> WriteFinish for zstd::Encoder<'_, W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<dyn io::Write>)
    }
}

#[cfg(feature = "gz")]
impl<W: io::Write> WriteFinish for flate2::write::GzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<dyn io::Write>)
    }
}

#[cfg(feature = "bz2")]
impl<W: io::Write> WriteFinish for bzip2::write::BzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<dyn io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<dyn io::Write>)
    }
}

pub fn writer<F, O>(o: &OutputOptions, vars: &mut Vars, func: F) -> CliResult<O>
where
    F: FnOnce(&mut dyn FormatWriter, &mut dyn io::Write, &mut Vars) -> CliResult<O>,
{
    io_writer(o, |io_writer| {
        let mut w = from_format(&o.format, vars)?;
        func(&mut w, io_writer, vars)
    })
}

/// Creates an io::Write either in the main thread (no compression)
/// or in a background thread (if explicitly specified or writing to
/// compressed format).
pub fn io_writer<F, O>(o: &OutputOptions, func: F) -> CliResult<O>
where
    F: FnOnce(&mut dyn io::Write) -> CliResult<O>,
{
    if o.compression != Compression::None || o.threaded {
        let thread_bufsize = o
            .thread_bufsize
            .unwrap_or_else(|| o.compression.best_write_bufsize());

        thread_io::write::writer_init_finish(
            thread_bufsize,
            4,
            || {
                let mut writer = io_writer_from_kind(&o.kind)?;
                writer = compr_writer(writer, o.compression, o.compression_level)?;
                Ok(writer)
            },
            |mut w| func(&mut w),
            |w| w.finish()?.flush(),
        )
        .map(|(o, _)| o)
    } else {
        let mut writer = io_writer_from_kind(&o.kind)?;
        let o = func(&mut writer)?;
        writer.finish()?.flush()?;
        Ok(o)
    }
}

pub fn from_format<'a>(
    format: &OutFormat,
    vars: &mut Vars,
) -> CliResult<Box<dyn FormatWriter + 'a>> {
    vars.build(|b| {
        let out: Box<dyn FormatWriter> = match *format {
            OutFormat::Fasta {
                ref attrs,
                wrap_width,
            } => {
                let writer = fasta::FastaWriter::new(wrap_width);
                Box::new(attr::AttrWriter::new(writer, attrs, b)?)
            }
            OutFormat::Fastq { format, ref attrs } => {
                let writer = fastq::FastqWriter::new(format);
                Box::new(attr::AttrWriter::new(writer, attrs, b)?)
            }
            OutFormat::FaQual {
                ref attrs,
                wrap_width,
                ref qfile,
            } => {
                let writer = fa_qual::FaQualWriter::new(wrap_width, qfile)?;
                Box::new(attr::AttrWriter::new(writer, attrs, b)?)
            }
            OutFormat::Csv { delim, ref fields } => {
                Box::new(csv::CsvWriter::new(fields, delim, b)?)
            }
        };
        Ok(out)
    })
}

pub fn io_writer_from_kind(kind: &OutputKind) -> io::Result<Box<dyn WriteFinish>> {
    Ok(match *kind {
        OutputKind::Stdout => Box::new(io::BufWriter::new(STDOUT.lock())),
        OutputKind::File(ref p) => Box::new(io::BufWriter::new(File::create(p).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Error creating '{}': {}", p, e),
            )
        })?)),
    })
}

pub fn compr_writer(
    writer: Box<dyn WriteFinish>,
    compression: Compression,
    #[allow(unused_variables)] level: Option<u8>,
) -> io::Result<Box<dyn WriteFinish>> {
    Ok(match compression {
        #[cfg(feature = "gz")]
        Compression::Gzip => Box::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::new(u32::from(level.unwrap_or(6))),
        )),
        #[cfg(feature = "bz2")]
        Compression::Bzip2 => {
            let c = match level {
                Some(l) => bzip2::Compression::new(l as u32),
                _ => bzip2::Compression::default(),
            };
            Box::new(bzip2::write::BzEncoder::new(writer, c))
        }
        #[cfg(feature = "lz4")]
        Compression::Lz4 => Box::new(
            lz4::EncoderBuilder::new()
                .level(level.unwrap_or(0) as u32)
                .build(writer)?,
        ),
        #[cfg(feature = "zstd")]
        Compression::Zstd => Box::new(zstd::Encoder::new(writer, i32::from(level.unwrap_or(0)))?),
        Compression::None => writer,
    })
}
