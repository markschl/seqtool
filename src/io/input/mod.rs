use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::{fmt, str::FromStr};

use seq_io::policy::BufPolicy;
use thread_io;

use crate::error::{CliError, CliResult};
use crate::helpers::seqtype::SeqType;

use super::csv::{ColumnMapping, CsvReader};
use super::{
    fa_qual, fasta, fastq, CompressionFormat, FileInfo, FormatVariant, QualFormat, Record,
    SeqReader, DEFAULT_FORMAT, DEFAULT_INFIELDS, DEFAULT_IO_WRITER_BUFSIZE,
};

mod parallel_csv;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum InputKind {
    Stdin,
    File(PathBuf),
}

impl FromStr for InputKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "-" => Ok(InputKind::Stdin),
            _ => Ok(InputKind::File(PathBuf::from(s))),
        }
    }
}

impl fmt::Display for InputKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InputKind::Stdin => write!(f, "-"),
            InputKind::File(ref p) => write!(f, "{}", p.as_path().to_string_lossy()),
        }
    }
}

impl InputKind {
    pub fn get_info(&self) -> FileInfo {
        match self {
            InputKind::Stdin => FileInfo::new(DEFAULT_FORMAT, None),
            InputKind::File(path) => FileInfo::from_path(path, DEFAULT_FORMAT, true),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct InputConfig {
    pub kind: InputKind,
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
        let strategy = LimitedBuffer {
            double_until: 1 << 23,
            limit: self.max_mem,
        };
        Ok(match self.format {
            InFormat::Fasta => Box::new(fasta::FastaReader::new(io_rdr, self.cap, strategy)),
            InFormat::Fastq { .. } => Box::new(fastq::FastqReader::new(io_rdr, self.cap, strategy)),
            InFormat::FaQual { ref qfile } => Box::new(fa_qual::FaQualReader::new(
                io_rdr, self.cap, strategy, qfile,
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
    /// back-transforms to FormatVariant (used for deriving output format from input format)
    pub fn format_variant(&self) -> FormatVariant {
        match *self {
            InFormat::Fasta => FormatVariant::Fasta,
            InFormat::Fastq { format } => FormatVariant::Fastq(format),
            InFormat::DelimitedText { delim, .. } => {
                if delim == b'\t' {
                    FormatVariant::Tsv
                } else {
                    FormatVariant::Csv
                }
            }
            InFormat::FaQual { .. } => FormatVariant::Fasta,
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
            FormatVariant::Csv => InFormat::DelimitedText {
                delim: text_delim.unwrap_or(',') as u8,
                fields: get_delim_fields(text_fields),
                has_header,
            },
            FormatVariant::Tsv => InFormat::DelimitedText {
                delim: text_delim.unwrap_or('\t') as u8,
                fields: get_delim_fields(text_fields),
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

pub fn get_delim_fields(
    fields: Option<&[(String, super::csv::TextColumnSpec)]>,
) -> Vec<(String, super::csv::TextColumnSpec)> {
    fields.map(|f| f.to_vec()).unwrap_or_else(|| {
        DEFAULT_INFIELDS
            .into_iter()
            .map(|(f, col)| (f.to_string(), col))
            .collect()
    })
}

#[derive(Clone)]
pub struct LimitedBuffer {
    double_until: usize,
    limit: usize,
}

impl BufPolicy for LimitedBuffer {
    fn grow_to(&mut self, current_size: usize) -> Option<usize> {
        if current_size < self.double_until {
            Some(current_size * 2)
        } else if current_size < self.limit {
            Some(current_size + self.double_until)
        } else {
            None
        }
    }
}

pub fn get_io_reader<'a>(
    kind: &InputKind,
    compression: Option<CompressionFormat>,
) -> CliResult<Box<dyn io::Read + Send + 'a>> {
    let rdr: Box<dyn io::Read + Send> = match kind {
        InputKind::File(ref path) => Box::new(
            File::open(path)
                .map_err(|e| format!("Error opening '{}': {}", path.to_string_lossy(), e))?,
        ),
        InputKind::Stdin => Box::new(io::stdin()),
    };
    if let Some(fmt) = compression {
        return Ok(get_compr_reader(rdr, fmt)?);
    }
    Ok(rdr)
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

pub fn with_io_reader<F, O>(o: &InputConfig, func: F) -> CliResult<O>
where
    for<'a> F: FnOnce(Box<dyn io::Read + Send + 'a>) -> CliResult<O>,
{
    let rdr = get_io_reader(&o.kind, o.compression)?;
    if o.compression.is_some() || o.threaded {
        // read in different thread
        let thread_bufsize = o.thread_bufsize.unwrap_or_else(|| {
            o.compression
                .map(|c| c.recommended_read_bufsize())
                .unwrap_or(DEFAULT_IO_WRITER_BUFSIZE)
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
            get_seq_reader(get_io_reader(&in_opts.kind, in_opts.compression)?, seq_opts)
        })
        .collect::<CliResult<_>>()?;

    let mut current_rec_id = Vec::new();
    'outer: loop {
        for (i, rdr) in readers.iter_mut().enumerate() {
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
            if let Some(res) = res {
                if !res? {
                    return Ok(());
                }
            } else {
                break 'outer;
            }
        }
    }
    // check if all readers are exhausted
    for rdr in readers.iter_mut() {
        if rdr.read_next(&mut |_| Ok(true)).is_none() {
            return fail!("");
        }
    }
    Ok(())
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
        } => parallel_csv::parallel_csv_init(
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
