use std::io;
use std::path::PathBuf;
use std::fs::File;

use flate2;
use bzip2;
use lz4;

use error::{CliError, CliResult};
use lib::thread_io;

use super::{fasta, fastq, Compression, Record, SeqWriter};

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
    pub compression: Option<Compression>,
    pub threaded: bool,
    pub thread_bufsize: usize,
}

impl Default for OutputOptions {
    fn default() -> OutputOptions {
        OutputOptions {
            kind: OutputKind::Stdout,
            format: OutFormat::FASTA(vec![], None),
            compression: None,
            threaded: false,
            thread_bufsize: 1 << 22,
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
    // q64, wrap_width, Vec<(attr_name, attr_value)>, default_seqattr_for_attrs
    FASTA(Vec<(String, String)>, Option<usize>),
    FASTQ(Vec<(String, String)>),
    //    FA_QUAL(PathBuf),
    CSV(u8, Vec<String>),
}

impl OutFormat {
    pub fn default_ext(&self) -> &'static str {
        match *self {
            OutFormat::FASTA(..) => "fasta",
            OutFormat::FASTQ(..) => "fastq",
            OutFormat::CSV(delim, _) => if delim == b'\t' {
                "txt"
            } else {
                "csv"
            },
        }
    }
}

pub fn writer<F, O>(opts: Option<&OutputOptions>, func: F) -> CliResult<O>
where
    F: FnOnce(&mut Writer) -> CliResult<O>,
{
    if let Some(o) = opts {
        io_writer_compr(&o.kind, o.compression, o.threaded, o.thread_bufsize, |io_writer| {
            let mut w = from_format(io_writer, &o.format)?;
            func(&mut w)
        })
    } else {
        func(&mut NoOutput)
    }
}

pub fn io_writer<F, O>(opts: Option<&OutputOptions>, func: F) -> CliResult<O>
where
    F: FnOnce(&mut io::Write) -> CliResult<O>,
{
    if let Some(o) = opts {
        io_writer_compr(&o.kind, o.compression, o.threaded, o.thread_bufsize, func)
    } else {
        func(&mut io::sink())
    }
}

pub fn from_format<'a, W>(io_writer: W, format: &OutFormat) -> CliResult<Box<Writer + 'a>>
where
    W: io::Write + 'a,
{
    Ok(match *format {
        OutFormat::FASTA(ref attrs, ref wrap) => {
            let writer = fasta::FastaWriter::new(io_writer, *wrap);
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::FASTQ(ref attrs) => {
            let writer = fastq::FastqWriter::new(io_writer);
            // if q64 {
            //     writer = writer.q64();
            // }
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::CSV(delim, ref fields) => {
            Box::new(csv::CsvWriter::new(io_writer, fields.clone(), delim))
        }
    })
}

pub fn from_kind(kind: &OutputKind) -> io::Result<Box<io::Write>> {
    Ok(match *kind {
        OutputKind::Stdout => Box::new(io::BufWriter::new(STDOUT.lock())),
        OutputKind::File(ref p) => Box::new(io::BufWriter::new(File::create(p)?)),
    })
}

pub fn compr_writer(
    writer: Box<io::Write>,
    compression: Compression,
) -> io::Result<Box<io::Write>> {
    Ok(match compression {
        Compression::GZIP => Box::new(flate2::write::GzEncoder::new(
            writer,
            flate2::Compression::default(),
        )),
        Compression::BZIP2 => Box::new(bzip2::write::BzEncoder::new(
            writer,
            bzip2::Compression::Default,
        )),
        Compression::LZ4 => Box::new(lz4::EncoderBuilder::new().build(writer)?),
    })
}

fn io_writer_compr<F, O>(
    kind: &OutputKind,
    compr: Option<Compression>,
    threaded: bool,
    thread_bufsize: usize,
    func: F,
) -> CliResult<O>
where
    F: FnOnce(&mut io::Write) -> CliResult<O>,
{
    if compr.is_some() || threaded {
        thread_io::write::writer_with(
            thread_bufsize, 4,
            || {
                let mut writer = from_kind(kind)?;
                if let Some(compr) = compr {
                    writer = compr_writer(writer, compr)?;
                }
                Ok::<_, CliError>(writer)
            },
            |mut w| func(&mut w),
        ).unwrap()
    } else {
        let mut writer = from_kind(kind)?;
        func(&mut writer)
    }
}
