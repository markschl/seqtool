use std::io;
use std::path::PathBuf;
use std::fs::File;

use flate2;
use bzip2;
use lz4;
use zstd;

use error::{CliError, CliResult};
use thread_io;

use super::{fasta, fastq, Compression, Record};

pub use self::writer::*;

pub mod attr;
pub mod csv;
pub mod writer;

lazy_static! {
    static ref STDOUT: io::Stdout = io::stdout();
}


pub trait SeqWriter<W: io::Write> {
    fn write(&mut self, record: &Record) -> CliResult<()>;
    fn into_inner(self: Box<Self>) -> Option<CliResult<W>>;
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
            format: OutFormat::FASTA(vec![], None),
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

/// Required by compression format encoders
pub trait WriteFinish: io::Write {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a;
}

impl<W: io::Write> WriteFinish for io::BufWriter<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a {
        Ok(self)
    }
}

impl<W: io::Write> WriteFinish for lz4::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a {
        let (w, res) = (*self).finish();
        res.map(|_| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for zstd::Encoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for flate2::write::GzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}

impl<W: io::Write> WriteFinish for bzip2::write::BzEncoder<W> {
    fn finish<'a>(self: Box<Self>) -> io::Result<Box<io::Write+'a>> where Self: 'a {
        (*self).finish().map(|w| Box::new(w) as Box<io::Write>)
    }
}



pub fn writer<'a, 'b, F, O>(opts: Option<&OutputOptions>, func: F) -> CliResult<O>
where
    F: FnOnce(&mut Writer<&mut io::Write>) -> CliResult<O>,
{
    if let Some(o) = opts {
        io_writer_compr(&o.kind, o.compression, o.compression_level, o.threaded, o.thread_bufsize,
            |io_writer| {
                let mut w = from_format(io_writer, &o.format)?;
                func(&mut w)
            }
        )
    } else {
        func(&mut NoOutput)
    }
}

pub fn io_writer<F, O>(opts: Option<&OutputOptions>, func: F) -> CliResult<O>
where
    F: FnOnce(&mut io::Write) -> CliResult<O>,
{
    if let Some(o) = opts {
        io_writer_compr(
            &o.kind, o.compression, o.compression_level, o.threaded, o.thread_bufsize, func
        )
    } else {
        func(&mut io::sink())
    }
}

pub fn from_format<'a, W>(io_writer: W, format: &OutFormat) -> CliResult<Box<Writer<W> + 'a>>
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
            Box::new(attr::AttrWriter::new(writer, attrs.clone()))
        }
        OutFormat::CSV(delim, ref fields) => {
            Box::new(csv::CsvWriter::new(io_writer, fields.clone(), delim))
        }
    })
}

pub fn io_writer_from_kind(kind: &OutputKind) -> io::Result<Box<WriteFinish>> {
    Ok(match *kind {
        OutputKind::Stdout => Box::new(io::BufWriter::new(STDOUT.lock())),
        OutputKind::File(ref p) => Box::new(io::BufWriter::new(
            File::create(p)
                .map_err(|e| io::Error::new(io::ErrorKind::Other,
                    format!("Error creating '{}': {}", p.to_string_lossy(), e)))?
        )),
    })
}


pub fn compr_writer(
    writer: Box<WriteFinish>,
    compression: Compression,
    level: Option<u8>,
) -> io::Result<Box<WriteFinish>>
{
    Ok(match compression {
        Compression::GZIP =>
            Box::new(flate2::write::GzEncoder::new(
                writer,
                flate2::Compression::new(level.unwrap_or(6) as u32),
            )
        ),
        Compression::BZIP2 => {
            let c = match level {
                Some(0...3) => bzip2::Compression::Fastest,
                Some(4...7) | None => bzip2::Compression::Default,
                Some(8...9) => bzip2::Compression::Best,
                _ => bzip2::Compression::Default,
            };
            Box::new(bzip2::write::BzEncoder::new(writer, c))
        },
        Compression::LZ4 =>
            Box::new(lz4::EncoderBuilder::new().build(writer)?),
        Compression::ZSTD=>
            Box::new(zstd::Encoder::new(writer, level.unwrap_or(0) as i32)?),
        Compression::None => writer,
    })
}

fn io_writer_compr<F, O>(
    kind: &OutputKind,
    compr: Compression,
    compr_level: Option<u8>,
    threaded: bool,
    thread_bufsize: Option<usize>,
    func: F,
) -> CliResult<O>
where
    F: FnOnce(&mut io::Write) -> CliResult<O>,
{
    if compr != Compression::None || threaded {

        let thread_bufsize = thread_bufsize.unwrap_or(compr.best_write_bufsize());

        thread_io::write::writer_init_finish(
            thread_bufsize, 4,
            || {
                let mut writer = io_writer_from_kind(kind)?;
                writer = compr_writer(writer, compr, compr_level)?;
                Ok(writer)
            },
            |mut w| func(&mut w),
            |w| w.finish()?.flush()
        ).map(|(o, _)| o)

    } else {
        let mut writer = io_writer_from_kind(kind)?;
        let o = func(&mut writer)?;
        writer.finish()?.flush()?;
        Ok(o)
    }
}
