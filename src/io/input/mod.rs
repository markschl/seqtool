use std::fmt;
use std::fs::File;
use std::io;
use std::path::PathBuf;

use bzip2::read::MultiBzDecoder;
use flate2::read::MultiGzDecoder;
use lz4;
use seq_io;
use seq_io::policy::BufPolicy;
use thread_io;
use zstd;

use super::*;
use crate::error::{CliError, CliResult};
use crate::helpers::util;

#[allow(dead_code)]
mod parallel_csv;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum InputType {
    Stdin,
    File(PathBuf),
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InputType::Stdin => write!(f, "-"),
            InputType::File(ref p) => write!(f, "{}", p.as_path().to_string_lossy()),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct InputOptions {
    pub kind: InputType,
    pub format: InFormat,
    pub compression: Compression,
    // read in separate thread
    pub threaded: bool,
    pub cap: usize,
    pub thread_bufsize: Option<usize>,
    pub max_mem: usize,
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
    Csv {
        delim: u8,
        fields: Vec<String>,
        has_header: bool,
    },
}

impl InFormat {
    pub fn name(&self) -> &'static str {
        match *self {
            InFormat::Fasta => "fasta",
            InFormat::Fastq { format } => match format {
                QualFormat::Sanger => "fastq",
                QualFormat::Illumina => "fastq-illumina",
                QualFormat::Solexa => "fastq-solexa",
                QualFormat::Phred => unreachable!(),
            },
            InFormat::FaQual { .. } => "<FASTA/QUAL>",
            InFormat::Csv { delim, .. } => {
                if delim == b'\t' {
                    "tsv"
                } else {
                    "csv"
                }
            }
        }
    }

    pub fn from_opts(
        string: &str,
        csv_delim: Option<&str>,
        csv_fields: Option<&str>,
        header: bool,
        qfile: Option<&str>,
    ) -> CliResult<InFormat> {
        let csv_fields = csv_fields
            .unwrap_or("id,desc,seq")
            .split(',')
            .map(|s| s.to_string())
            .collect();

        let format = match string {
            "fasta" | "fa" | "fna" | "<FASTA/QUAL>" => InFormat::Fasta,
            "fastq" | "fq" => InFormat::Fastq {
                format: QualFormat::Sanger,
            },
            "fastq-illumina" | "fq-illumina" => InFormat::Fastq {
                format: QualFormat::Illumina,
            },
            "fastq-solexa" | "fq-solexa" => InFormat::Fastq {
                format: QualFormat::Solexa,
            },
            "csv" => InFormat::Csv {
                delim: util::parse_delimiter(csv_delim.unwrap_or(","))?,
                fields: csv_fields,
                has_header: header,
            },
            "tsv" => InFormat::Csv {
                delim: util::parse_delimiter(csv_delim.unwrap_or("\t"))?,
                fields: csv_fields,
                has_header: header,
            },
            _ => {
                return Err(CliError::Other(format!(
                    "Unknown input format: '{}'.",
                    string
                )))
            }
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
            InFormat::Csv { fields, .. } => {
                fields.iter().any(|f| f.trim_start().starts_with("qual"))
            }
            _ => false,
        }
    }
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

fn get_io_reader<'a>(o: &InputOptions) -> CliResult<Box<dyn io::Read + Send + 'a>> {
    let rdr: Box<dyn io::Read + Send> = match o.kind {
        InputType::File(ref path) => Box::new(
            File::open(path)
                .map_err(|e| format!("Error opening '{}': {}", path.to_string_lossy(), e))?,
        ),
        InputType::Stdin => Box::new(io::stdin()),
    };
    get_compr_reader(rdr, o.compression).map_err(From::from)
}

fn get_compr_reader<'a>(
    rdr: Box<dyn io::Read + Send + 'a>,
    compression: Compression,
) -> io::Result<Box<dyn io::Read + Send + 'a>> {
    Ok(match compression {
        Compression::Gzip => Box::new(MultiGzDecoder::new(rdr)),
        Compression::Bzip2 => Box::new(MultiBzDecoder::new(rdr)),
        Compression::Lz4 => Box::new(lz4::Decoder::new(rdr)?),
        Compression::Zstd => Box::new(zstd::Decoder::new(rdr)?),
        Compression::None => rdr,
    })
}

fn io_reader<F, O>(o: &InputOptions, func: F) -> CliResult<O>
where
    for<'a> F: FnOnce(Box<dyn io::Read + Send + 'a>) -> CliResult<O>,
{
    let rdr = get_io_reader(o)?;
    if o.compression != Compression::None || o.threaded {
        // read in different thread
        let thread_bufsize = o
            .thread_bufsize
            .unwrap_or_else(|| o.compression.best_read_bufsize());
        thread_io::read::reader(thread_bufsize, 2, rdr, |r| func(Box::new(r)))
    } else {
        func(rdr)
    }
}

pub fn io_readers<'a, I, F, O>(opts: I, mut func: F) -> CliResult<Vec<O>>
where
    I: IntoIterator<Item = &'a InputOptions>,
    for<'b> F: FnMut(&InputOptions, Box<dyn io::Read + Send + 'b>) -> CliResult<O>,
{
    opts.into_iter()
        .map(|o| io_reader(&o, |rdr| func(o, rdr)))
        .collect()
}

pub fn read_parallel<W, S, Si, Di, F, D, R>(
    o: &InputOptions,
    rdr: R,
    n_threads: u32,
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
        run_reader(rdr, &o.format, o.cap, o.max_mem, &mut |record| {
            work(record, &mut out, &mut rset_data)?;
            func(record, &mut out)
        })
    } else {
        run_reader_parallel(
            &o.format,
            rdr,
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
pub fn run_reader<R>(
    rdr: R,
    format: &InFormat,
    cap: usize,
    max_mem: usize,
    func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
) -> CliResult<()>
where
    R: io::Read,
{
    let mut rdr = get_reader(rdr, format, cap, max_mem)?;
    while let Some(res) = rdr.read_next(func) {
        if !res?? {
            break;
        }
    }
    Ok(())
}

pub fn read_alongside<'a, I, F>(opts: I, mut func: F) -> CliResult<()>
where
    I: IntoIterator<Item = &'a InputOptions>,
    F: FnMut(usize, &dyn Record) -> CliResult<()>,
{
    let mut readers: Vec<_> = opts
        .into_iter()
        .map(|o| get_reader(get_io_reader(o)?, &o.format, o.cap, o.max_mem))
        .collect::<CliResult<_>>()?;

    loop {
        for (i, rdr) in readers.iter_mut().enumerate() {
            if let Some(res) = rdr.read_next(&mut |rec| func(i, rec)) {
                res??;
            } else {
                return Ok(());
            }
        }
    }
}

pub fn get_reader<'a, O, R>(
    rdr: R,
    format: &InFormat,
    cap: usize,
    max_mem: usize,
) -> CliResult<Box<dyn SeqReader<O> + 'a>>
where
    R: io::Read + 'a,
{
    let strategy = LimitedBuffer {
        double_until: 1 << 23,
        limit: max_mem,
    };
    Ok(match *format {
        InFormat::Fasta => Box::new(fasta::FastaReader::new(rdr, cap, strategy)),
        InFormat::Fastq { .. } => Box::new(fastq::FastqReader::new(rdr, cap, strategy)),
        InFormat::FaQual { ref qfile } => {
            Box::new(fa_qual::FaQualReader::new(rdr, cap, strategy, qfile)?)
        }
        InFormat::Csv {
            ref delim,
            ref fields,
            has_header,
        } => Box::new(csv::CsvReader::new(rdr, *delim, fields, has_header)?),
    })
}

// run reader in multiple threads
// contains format specific code
// should be nicer once one generic function can be used instead of
// multiple functions generated by parallel_record_impl!() (seq_io crate)
fn run_reader_parallel<R, Di, D, Si, S, W, F>(
    format: &InFormat,
    rdr: R,
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
                *delim = rec.delim();
            },
            |rec, &mut (ref mut d, delim), s| {
                let rec = fasta::FastaRecord::new(rec);
                if let Some(_d) = delim {
                    rec.set_delim(_d);
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
                *delim = rec.delim();
            },
            |rec, &mut (ref mut d, delim), s| {
                let rec = fastq::FastqRecord::new(rec);
                if let Some(_d) = delim {
                    rec.set_delim(_d);
                }
                transform_result!(func(&rec, d, s))
            },
        ),
        InFormat::FaQual { .. } => {
            return fail!(
                "Multithreaded processing of records with qualities from .qual files implemented"
            )
        }
        InFormat::Csv {
            ref delim,
            ref fields,
            has_header,
        } => parallel_csv::parallel_csv_init(
            n_threads,
            queue_len,
            || csv::CsvReader::new(rdr, *delim, fields, has_header),
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
