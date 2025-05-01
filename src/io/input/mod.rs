use std::fs::File;
use std::io;
use std::path::PathBuf;

use fastx::LimitedBuffer;
use thread_io;

use crate::error::{CliError, CliResult};
use crate::helpers::seqtype::SeqType;

use super::{
    parse_compr_ext, CompressionFormat, FormatVariant, IoKind, QualFormat, Record,
    DEFAULT_IO_READER_BUFSIZE,
};

use self::csv::{parallel_csv_init, ColumnMapping, CsvReader, TextColumnSpec, DEFAULT_INFIELDS};

pub mod csv;
pub mod fa_qual;
pub mod fasta;
pub mod fastq;
mod fastx;
mod reader;

pub use self::reader::*;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct InputConfig {
    pub kind: IoKind,
    pub compression: Option<CompressionFormat>,
    // read in separate thread
    pub threaded: bool,
    pub thread_bufsize: Option<usize>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct SeqReaderConfig {
    pub format: InFormat,
    pub seqtype: Option<SeqType>,
    /// Buffer capacity (for FASTX readers)
    pub cap: usize,
    /// Maximum memory to use (for FASTX readers)
    pub max_mem: usize,
}

impl SeqReaderConfig {
    pub fn get_seq_reader<'a, R>(&self, io_rdr: R) -> CliResult<Box<dyn SeqReader + 'a>>
    where
        R: io::Read + 'a,
    {
        let fastx_buf_strategy = LimitedBuffer {
            double_until: 1 << 23,
            limit: self.max_mem,
        };
        Ok(match self.format {
            InFormat::Fasta => Box::new(fasta::FastaReader::new(
                io_rdr,
                self.cap,
                fastx_buf_strategy,
            )),
            InFormat::Fastq { .. } => Box::new(fastq::FastqReader::new(
                io_rdr,
                self.cap,
                fastx_buf_strategy,
            )),
            InFormat::FaQual { ref qfile } => Box::new(fa_qual::FaQualReader::new(
                io_rdr,
                self.cap,
                fastx_buf_strategy,
                qfile,
            )?),
            InFormat::DelimitedText {
                ref delim,
                ref fields,
                has_header,
            } => Box::new(CsvReader::new(io_rdr, *delim, fields, has_header)?),
        })
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum InFormat {
    Fasta,
    Fastq {
        format: QualFormat,
    },
    FaQual {
        qfile: PathBuf,
    },
    DelimitedText {
        delim: u8,
        fields: Vec<ColumnMapping>,
        has_header: bool,
    },
}

impl InFormat {
    /// Returns the `FormatVariant` and options relevant for the ouptut format
    #[allow(clippy::type_complexity)]
    pub fn components(
        &self,
    ) -> (
        FormatVariant,
        Option<&[(String, TextColumnSpec)]>,
        Option<char>,
    ) {
        match *self {
            InFormat::Fasta => (FormatVariant::Fasta, None, None),
            InFormat::Fastq { format } => (FormatVariant::Fastq(format), None, None),
            InFormat::DelimitedText {
                delim, ref fields, ..
            } => {
                let fmt = if delim == b'\t' {
                    FormatVariant::Tsv
                } else {
                    FormatVariant::Csv
                };
                (fmt, Some(fields), Some(delim as char))
            }
            InFormat::FaQual { .. } => (FormatVariant::Fasta, None, None),
        }
    }

    pub fn from_opts(
        format: FormatVariant,
        text_delim: Option<char>,
        text_fields: Option<&[ColumnMapping]>,
        has_header: bool,
        qfile: Option<&str>,
    ) -> CliResult<InFormat> {
        let format = match format {
            FormatVariant::Fasta => InFormat::Fasta,
            FormatVariant::Fastq(format) => InFormat::Fastq { format },
            FormatVariant::Csv | FormatVariant::Tsv => InFormat::DelimitedText {
                delim: text_delim.unwrap_or_else(|| {
                    if format == FormatVariant::Csv {
                        ','
                    } else {
                        '\t'
                    }
                }) as u8,
                fields: text_fields.map(|f| f.to_vec()).unwrap_or_else(|| {
                    DEFAULT_INFIELDS
                        .into_iter()
                        .map(|(f, col)| (f.to_string(), col))
                        .collect()
                }),
                has_header,
            },
        };

        if let Some(f) = qfile {
            if format != InFormat::Fasta {
                return fail!("Expecting FASTA as input if combined with QUAL files");
            }
            return Ok(InFormat::FaQual {
                qfile: PathBuf::from(f),
            });
        }

        Ok(format)
    }

    pub fn has_qual(&self) -> bool {
        match self {
            InFormat::Fastq { .. } | InFormat::FaQual { .. } => true,
            InFormat::DelimitedText { fields, .. } => fields.iter().any(|(f, _)| f == "qual"),
            _ => false,
        }
    }
}

impl IoKind {
    /// Infers the input compression and sequence format from the path extension.
    /// Applies the default format (along with a message) if the format is unknown.
    pub fn infer_in_format(
        &self,
        default_format: FormatVariant,
    ) -> (FormatVariant, Option<CompressionFormat>) {
        match self {
            IoKind::Stdio => (default_format, None),
            IoKind::File(path) => {
                let (compression, ext) = parse_compr_ext(&path);
                let format = ext.and_then(FormatVariant::str_match).unwrap_or_else(|| {
                    eprintln!(
                        "{} extension for file '{}' assuming the '{}' format",
                        if ext.is_none() { "No" } else { "Unknown" },
                        path.to_string_lossy(),
                        default_format
                    );
                    default_format
                });
                (format, compression)
            }
        }
    }

    pub fn io_reader_with_compression(
        &self,
        compression: Option<CompressionFormat>,
    ) -> CliResult<Box<dyn io::Read + Send>> {
        let rdr: Box<dyn io::Read + Send> = match self {
            IoKind::File(ref path) => Box::new(
                File::open(path)
                    .map_err(|e| format!("Error opening '{}': {}", path.to_string_lossy(), e))?,
            ),
            IoKind::Stdio => Box::new(io::stdin()),
        };
        if let Some(fmt) = compression {
            return Ok(get_compr_reader(rdr, fmt)?);
        }
        Ok(rdr)
    }

    /// Returns an I/O reader, auto-recognizing compression formats from the file extension.
    /// Returns the file extension if present.
    pub fn io_reader(&self) -> CliResult<(Box<dyn io::Read + Send>, Option<String>)> {
        let (compr, ext) = match self {
            IoKind::File(ref path) => parse_compr_ext(path),
            IoKind::Stdio => (None, None),
        };
        let rdr = self.io_reader_with_compression(compr)?;
        Ok((rdr, ext.map(|e| e.to_string())))
    }
}

fn get_compr_reader<'a>(
    rdr: Box<dyn io::Read + Send + 'a>,
    compression: CompressionFormat,
) -> io::Result<Box<dyn io::Read + Send + 'a>> {
    Ok(match compression {
        #[cfg(feature = "gz")]
        CompressionFormat::Gzip => Box::new(flate2::read::MultiGzDecoder::new(rdr)),
        #[cfg(feature = "bz2")]
        CompressionFormat::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(rdr)),
        #[cfg(feature = "lz4")]
        CompressionFormat::Lz4 => Box::new(lz4::Decoder::new(rdr)?),
        #[cfg(feature = "zstd")]
        CompressionFormat::Zstd => Box::new(zstd::Decoder::new(rdr)?),
    })
}

pub fn thread_reader<F, O>(o: &InputConfig, func: F) -> CliResult<O>
where
    for<'a> F: FnOnce(Box<dyn io::Read + Send + 'a>) -> CliResult<O>,
{
    let rdr = o.kind.io_reader_with_compression(o.compression)?;
    if o.compression.is_some() || o.threaded {
        // read in different thread
        let thread_bufsize = o.thread_bufsize.unwrap_or_else(|| {
            o.compression
                .map(|c| c.recommended_read_bufsize())
                .unwrap_or(DEFAULT_IO_READER_BUFSIZE)
        });
        thread_io::read::reader(thread_bufsize, 2, rdr, |r| func(Box::new(r)))
    } else {
        func(rdr)
    }
}

pub fn read_parallel<W, S, Si, Di, F, D, R>(
    io_rdr: R,
    n_threads: u32,
    opts: &SeqReaderConfig,
    rset_data_init: Si,
    record_data_init: Di,
    work: W,
    mut func: F,
) -> CliResult<()>
where
    W: Fn(&dyn Record, &mut D, &mut S) -> CliResult<()> + Send + Sync,
    F: FnMut(&dyn Record, &mut D) -> CliResult<bool>,
    R: io::Read + Send,
    Di: Fn() -> D + Send + Sync,
    D: Send,
    S: Send,
    Si: Fn() -> CliResult<S> + Send + Sync,
{
    if n_threads <= 1 {
        let mut out = record_data_init();
        let mut rset_data = rset_data_init()?;
        read(io_rdr, opts, &mut |record| {
            work(record, &mut out, &mut rset_data)?;
            func(record, &mut out)
        })
    } else {
        run_reader_parallel(
            io_rdr,
            &opts.format,
            n_threads,
            || Ok((record_data_init(), None::<CliError>)),
            &rset_data_init,
            |rec, &mut (ref mut out, ref mut res), l| {
                *res = work(rec, out, l).err();
            },
            |rec, &mut (ref mut out, ref mut res), _| {
                if let Some(e) = res.take() {
                    return Err(e);
                }
                func(rec, out)
            },
        )
    }
}

// Run reader in single thread
pub fn read<R>(
    io_rdr: R,
    opts: &SeqReaderConfig,
    func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
) -> CliResult<()>
where
    R: io::Read,
{
    let mut rdr = get_seq_reader(io_rdr, opts)?;
    while let Some(res) = rdr.read_next(func) {
        if !res? {
            break;
        }
    }
    Ok(())
}

pub fn read_alongside<'a, I, F>(opts: I, id_check: bool, mut func: F) -> CliResult<()>
where
    I: IntoIterator<Item = &'a (InputConfig, SeqReaderConfig)>,
    F: FnMut(usize, &dyn Record) -> CliResult<bool>,
{
    let mut readers: Vec<_> = opts
        .into_iter()
        .map(|(in_opts, seq_opts)| {
            get_seq_reader(
                in_opts
                    .kind
                    .io_reader_with_compression(in_opts.compression)?,
                seq_opts,
            )
        })
        .collect::<CliResult<_>>()?;

    let mut current_rec_id = Vec::new();
    let i = 'err: loop {
        let mut readers_iter = readers.iter_mut().enumerate();
        while let Some((i, rdr)) = readers_iter.next() {
            let res = rdr.read_next(&mut |rec| {
                if id_check {
                    let rec_id = rec.id();
                    if i == 0 {
                        current_rec_id.clear();
                        current_rec_id.extend(rec_id);
                    } else if rec_id != current_rec_id.as_slice() {
                        return fail!(format!(
                            "ID of record #{} ({}) does not match the ID of the first one ({})",
                            i + 1,
                            String::from_utf8_lossy(rec_id),
                            String::from_utf8_lossy(&current_rec_id)
                        ));
                    }
                }
                func(i, rec)
            });
            let finished = if let Some(res) = res { !res? } else { true };
            if finished {
                if i != 0 {
                    break 'err i;
                }
                for (i, rdr) in readers_iter {
                    if rdr.read_next(&mut |_| Ok(true)).is_some() {
                        break 'err i;
                    }
                }
                return Ok(());
            }
        }
    };
    fail!(
        "The number of records in input #{} does not match \
        the number of records in input #1",
        i + 1
    )
}

pub fn get_seq_reader<'a, R>(
    io_rdr: R,
    opts: &SeqReaderConfig,
) -> CliResult<Box<dyn SeqReader + 'a>>
where
    R: io::Read + 'a,
{
    let strategy = LimitedBuffer {
        double_until: 1 << 23,
        limit: opts.max_mem,
    };
    Ok(match opts.format {
        InFormat::Fasta => Box::new(fasta::FastaReader::new(io_rdr, opts.cap, strategy)),
        InFormat::Fastq { .. } => Box::new(fastq::FastqReader::new(io_rdr, opts.cap, strategy)),
        InFormat::FaQual { ref qfile } => Box::new(fa_qual::FaQualReader::new(
            io_rdr, opts.cap, strategy, qfile,
        )?),
        InFormat::DelimitedText {
            ref delim,
            ref fields,
            has_header,
        } => Box::new(CsvReader::new(io_rdr, *delim, fields, has_header)?),
    })
}

// run reader in multiple threads
// contains format specific code
// should be nicer once one generic function can be used instead of
// multiple functions generated by parallel_record_impl!() (seq_io crate)
fn run_reader_parallel<R, Di, D, Si, S, W, F>(
    rdr: R,
    format: &InFormat,
    n_threads: u32,
    record_data_init: Di,
    rset_data_init: Si,
    work: W,
    mut func: F,
) -> CliResult<()>
where
    R: io::Read + Send,
    Di: Fn() -> CliResult<D> + Send + Sync,
    D: Send,
    Si: Fn() -> CliResult<S> + Send + Sync,
    S: Send,
    W: Fn(&dyn Record, &mut D, &mut S) + Send + Sync,
    F: FnMut(&dyn Record, &mut D, &mut S) -> CliResult<bool>,
{
    // not very nice, but saves some repetitition
    macro_rules! transform_result {
        ($res:expr) => {{
            match $res {
                Ok(res) => {
                    if !res {
                        return Some(Ok(()));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
            None
        }};
    }

    let queue_len = n_threads as usize * 2;

    let out: CliResult<Option<CliResult<()>>> = match *format {
        InFormat::Fasta => seq_io::parallel::parallel_fasta_init(
            n_threads,
            queue_len,
            || Ok::<_, seq_io::fasta::Error>(seq_io::fasta::Reader::new(rdr)),
            || record_data_init().map(|d| (d, None)),
            rset_data_init,
            |rec, &mut (ref mut d, ref mut delim), s| {
                let rec = fasta::FastaRecord::new(rec);
                work(&rec as &dyn Record, d, s);
                *delim = rec.header_delim_pos(); // cache the delimiter position (if known)
            },
            |rec, &mut (ref mut d, delim), s| {
                let rec = fasta::FastaRecord::new(rec);
                if let Some(_d) = delim {
                    rec.set_header_delim_pos(_d);
                }
                transform_result!(func(&rec, d, s))
            },
        ),
        InFormat::Fastq { .. } => seq_io::parallel::parallel_fastq_init(
            n_threads,
            queue_len,
            || Ok::<_, seq_io::fastq::Error>(seq_io::fastq::Reader::new(rdr)),
            || record_data_init().map(|d| (d, None)),
            rset_data_init,
            |rec, &mut (ref mut d, ref mut delim), s| {
                let rec = fastq::FastqRecord::new(rec);
                work(&rec as &dyn Record, d, s);
                *delim = rec.header_delim_pos(); // apply delimiter position (if known)
            },
            |rec, &mut (ref mut d, delim), s| {
                let rec = fastq::FastqRecord::new(rec);
                if let Some(_d) = delim {
                    rec.set_header_delim_pos(_d); // apply delimiter position (if known)
                }
                transform_result!(func(&rec, d, s))
            },
        ),
        InFormat::FaQual { .. } => {
            return fail!(
                "Multithreaded processing of records with qualities from .qual files implemented"
            )
        }
        InFormat::DelimitedText {
            ref delim,
            ref fields,
            has_header,
        } => parallel_csv_init(
            n_threads,
            queue_len,
            || CsvReader::new(rdr, *delim, fields, has_header),
            record_data_init,
            rset_data_init,
            |rec, d, s| work(&rec as &dyn Record, d, s),
            |rec, d, s| transform_result!(func(&rec as &dyn Record, d, s)),
        ),
    };
    match out? {
        Some(Err(e)) => Err(e),
        _ => Ok(()),
    }
}
