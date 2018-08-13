use std::fs::File;
use std::io;
use std::path::PathBuf;

use bzip2;
use flate2;
use lz4;
use zstd;

use error::{CliError, CliResult};
use lib::util;
use thread_io;

use super::input::InFormat;
use super::{fa_qual, fasta, fastq, Compression, QualFormat, Record};

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

impl Default for OutputOptions {
    fn default() -> OutputOptions {
        OutputOptions {
            kind: OutputKind::Stdout,
            format: OutFormat::FASTA {
                attrs: vec![],
                wrap_width: None,
            },
            compression: Compression::None,
            compression_level: None,
            threaded: false,
            thread_bufsize: None,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum OutputKind {
    Stdout,
    File(PathBuf),
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum OutFormat {
    FASTA {
        attrs: Vec<(String, String)>,
        // Vec<(attr_name, attr_value)>, default_seqattr_for_attrs
        wrap_width: Option<usize>,
    },
    FASTQ {
        // only Some() if different from input format
        format: Option<QualFormat>,
        attrs: Vec<(String, String)>,
    },
    FaQual {
        attrs: Vec<(String, String)>,
        wrap_width: Option<usize>,
        qfile: PathBuf,
    },
    CSV {
        delim: u8,
        fields: Vec<String>,
    },
}

impl OutFormat {
    pub fn default_ext(&self) -> &'static str {
        match *self {
            OutFormat::FASTA { .. } => "fasta",
            OutFormat::FASTQ { .. } => "fastq",
            OutFormat::FaQual { .. } => "fasta",
            OutFormat::CSV { delim, .. } => if delim == b'\t' {
                "tsv"
            } else {
                "csv"
            },
        }
    }

    pub fn from_opts(
        string: &str,
        attrs: &[(String, String)],
        wrap_fasta: Option<usize>,
        csv_delim: Option<&str>,
        csv_fields: &str,
        informat: Option<&InFormat>,
        qfile: Option<&str>,
    ) -> CliResult<OutFormat> {
        let csv_fields = csv_fields.split(',').map(|s| s.to_string()).collect();

        let mut format = match string {
            "fasta" | "fna" | "fa" | "<FASTA/QUAL>" => OutFormat::FASTA {
                attrs: attrs.to_owned(),
                wrap_width: wrap_fasta,
            },
            "fastq" | "fq" => OutFormat::FASTQ {
                format: Some(QualFormat::Sanger),
                attrs: attrs.to_owned(),
            },
            "fastq-illumina" | "fq-illumina" => OutFormat::FASTQ {
                format: Some(QualFormat::Illumina),
                attrs: attrs.to_owned(),
            },
            "fastq-solexa" | "fq-solexa" => OutFormat::FASTQ {
                format: Some(QualFormat::Solexa),
                attrs: attrs.to_owned(),
            },
            "csv" => OutFormat::CSV {
                delim: util::parse_delimiter(csv_delim.unwrap_or(","))?,
                fields: csv_fields,
            },
            "tsv" | "txt" => OutFormat::CSV {
                delim: util::parse_delimiter(csv_delim.unwrap_or("\t"))?,
                fields: csv_fields,
            },
            _ => {
                return Err(CliError::Other(format!(
                    "Unknown output format: '{}'",
                    string
                )))
            }
        };

        // remove quality output format if equal to input format
        if let OutFormat::FASTQ { format: outfmt, .. } = &mut format {
            if let Some(&InFormat::FASTQ { format: infmt }) = informat {
                if outfmt == &Some(infmt) {
                    *outfmt = None;
                }
            }
        }

        // FaQual format
        if let Some(f) = qfile {
            match format {
                OutFormat::FASTA { attrs, wrap_width } => {
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

/// Required by compression format encoders
pub trait WriteFinish: io::Write {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a;
}

impl<W: io::Write> WriteFinish for io::BufWriter<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a,
    {
        Ok(self)
    }
}

impl<W: io::Write> WriteFinish for lz4::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a,
    {
        let (w, res) = (*self).finish();
        res.map(|_| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for zstd::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for flate2::write::GzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for bzip2::write::BzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write + 'a>>
    where
        Self: 'a,
    {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}

pub fn writer<'a, 'b, F, O>(o: &OutputOptions, func: F) -> CliResult<O>
where
    F: FnOnce(&mut Writer<&mut io::Write>) -> CliResult<O>,
{
    io_writer(o, |io_writer| {
        let mut w = from_format(io_writer, &o.format)?;
        func(&mut w)
    })
}

pub fn io_writer<F, O>(o: &OutputOptions, func: F) -> CliResult<O>
where
    F: FnOnce(&mut io::Write) -> CliResult<O>,
{
    if o.compression != Compression::None || o.threaded {
        let thread_bufsize = o
            .thread_bufsize
            .unwrap_or(o.compression.best_write_bufsize());

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
        ).map(|(o, _)| o)
    } else {
        let mut writer = io_writer_from_kind(&o.kind)?;
        let o = func(&mut writer)?;
        writer.finish()?.flush()?;
        Ok(o)
    }
}

pub fn from_format<'a, W>(io_writer: W, format: &OutFormat) -> CliResult<Box<Writer<W> + 'a>>
where
    W: io::Write + 'a,
{
    Ok(match *format {
        OutFormat::FASTA {
            ref attrs,
            wrap_width,
        } => {
            let writer = fasta::FastaWriter::new(io_writer, wrap_width);
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::FASTQ { format, ref attrs } => {
            let writer = fastq::FastqWriter::new(io_writer, format);
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::FaQual {
            ref attrs,
            wrap_width,
            ref qfile,
        } => {
            let writer = fa_qual::FaQualWriter::new(io_writer, wrap_width, qfile)?;
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::CSV { delim, ref fields } => {
            Box::new(csv::CsvWriter::new(io_writer, fields.clone(), delim))
        }
    })
}

pub fn io_writer_from_kind(kind: &OutputKind) -> io::Result<Box<WriteFinish>> {
    Ok(match *kind {
        OutputKind::Stdout => Box::new(io::BufWriter::new(STDOUT.lock())),
        OutputKind::File(ref p) => Box::new(io::BufWriter::new(File::create(p).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Error creating '{}': {}", p.to_string_lossy(), e),
            )
        })?)),
    })
}

pub fn compr_writer(
    writer: Box<WriteFinish>,
    compression: Compression,
    level: Option<u8>,
) -> io::Result<Box<WriteFinish>> {
    Ok(match compression {
        Compression::GZIP => Box::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::new(level.unwrap_or(6) as u32),
        )),
        Compression::BZIP2 => {
            let c = match level {
                Some(0...3) => bzip2::Compression::Fastest,
                Some(4...7) | None => bzip2::Compression::Default,
                Some(8...9) => bzip2::Compression::Best,
                _ => bzip2::Compression::Default,
            };
            Box::new(bzip2::write::BzEncoder::new(writer, c))
        }
        Compression::LZ4 => Box::new(lz4::EncoderBuilder::new().build(writer)?),
        Compression::ZSTD => Box::new(zstd::Encoder::new(writer, level.unwrap_or(0) as i32)?),
        Compression::None => writer,
    })
}
