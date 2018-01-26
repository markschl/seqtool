use std::io;
use std::fs::File;
use std::path::PathBuf;
use std::fmt;

use seq_io;
use flate2::read::GzDecoder;
use bzip2::read::BzDecoder;
use lz4;
use zstd;

use error::CliResult;
use thread_io;
use super::{csv, fasta, fastq, Compression, Record, SeqReader};
use lib::util;

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
    pub compression: Option<Compression>,
    // read in separate thread
    pub threaded: bool,
    pub qfile: Option<PathBuf>,
    pub cap: usize,
    pub thread_bufsize: Option<usize>,
    pub max_mem: usize,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum InFormat {
    FASTA,
    FASTQ,
    CSV(u8, Vec<String>, bool),
}

impl InFormat {
    pub fn name(&self) -> &'static str {
        match *self {
            InFormat::FASTA => "fasta",
            InFormat::FASTQ => "fastq",
            InFormat::CSV(d, _, _) => if d == b'\t' {
                "txt"
            } else {
                "csv"
            },
        }
    }

    pub fn from_opts(
        string: &str,
        csv_delim: Option<&str>,
        csv_fields: &str,
        header: bool,
    ) -> CliResult<InFormat> {
        let csv_fields = csv_fields.split(',').map(|s| s.to_string()).collect();

        let format = match string {
            "fasta" => InFormat::FASTA,
            "fastq" => InFormat::FASTQ,
            "csv" => InFormat::CSV(
                util::parse_delimiter(csv_delim.unwrap_or(","))?,
                csv_fields,
                header,
            ),
            "txt" => InFormat::CSV(
                util::parse_delimiter(csv_delim.unwrap_or("\t"))?,
                csv_fields,
                header,
            ),
            _ => {
                return Err(CliError::Other(format!(
                    "Unknown input format: '{}'.",
                    string
                )))
            }
        };

        Ok(format)
    }
}

pub struct LimitedBufStrategy {
    double_until: usize,
    limit: usize,
}

impl seq_io::BufStrategy for LimitedBufStrategy {
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

fn get_io_reader<'a>(kind: &'a InputType) -> CliResult<Box<io::Read + Send + 'a>> {
    Ok(match *kind {
        InputType::File(ref path) => Box::new(
            File::open(path)
                .map_err(|e| format!("Error opening '{}': {}", path.to_string_lossy(), e))?
        ),
        InputType::Stdin => Box::new(io::stdin()),
    })
}

fn get_compr_reader<'a, R>(
    rdr: R,
    compression: Compression,
) -> io::Result<Box<io::Read + Send + 'a>>
where
    R: io::Read + Send + 'a,
{
    Ok(match compression {
        Compression::GZIP  => Box::new(GzDecoder::new(rdr)),
        Compression::BZIP2 => Box::new(BzDecoder::new(rdr)),
        Compression::LZ4  => Box::new(lz4::Decoder::new(rdr)?),
        Compression::ZSTD => Box::new(zstd::Decoder::new(rdr)?),
    })
}

use error::CliError;

fn io_reader<F, O>(
    kind: &InputType,
    compression: Option<Compression>,
    threaded: bool,
    thread_bufsize: Option<usize>,
    func: F,
) -> CliResult<O>
where
    for<'b> F: FnOnce(Box<io::Read + Send + 'b>) -> CliResult<O>,
{
    let mut rdr = get_io_reader(kind)?;
    if compression.is_some() || threaded {
        // read in different thread
        let thread_bufsize =
            if let Some(compr) = compression {
                rdr = get_compr_reader(rdr, compr)?;
                thread_bufsize.unwrap_or(compr.best_read_bufsize())
            } else {
                thread_bufsize.unwrap_or(1 << 22)
            };
        thread_io::read::reader(
            thread_bufsize,
             2, rdr, |r| func(Box::new(r))
         )
    } else {
        func(rdr)
    }
}

pub fn io_readers<F, O>(opts: &[InputOptions], mut func: F) -> CliResult<Vec<O>>
where
    for<'b> F: FnMut(&InputOptions, Box<io::Read + Send + 'b>) -> CliResult<O>,
{
    opts.into_iter()
        .map(|o| io_reader(&o.kind, o.compression, o.threaded, o.thread_bufsize, |rdr| func(o, rdr)))
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
    W: Fn(&Record, &mut D, &mut S) -> CliResult<()> + Send + Sync,
    F: FnMut(&Record, &mut D) -> CliResult<bool>,
    R: io::Read + Send,
    Di: Fn() -> D + Send + Sync,
    D: Send,
    S: Send,
    Si: Fn() -> CliResult<S> + Send + Sync,
{
    if n_threads <= 1 {
        let mut out = record_data_init();
        let mut rset_data = rset_data_init()?;
        run_reader(&o.format, rdr, o.cap, o.max_mem, |record| {
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

macro_rules! run_rdr {
    ($rdr:expr, $func:expr, $rec:ident, $mod_rec:block) => {
        {
            while let Some(res) = (&mut $rdr).next() {
                let $rec = res?;
                let rec = $mod_rec;
                if ! $func(&rec)? {
                    break;
                }
            }
            Ok(())
        }
     };
}


pub fn run_reader<'a, R, F>(
    format: &InFormat,
    rdr: R,
    cap: usize,
    max_mem: usize,
    mut func: F,
) -> CliResult<()>
where
    R: io::Read + Send + 'a,
    F: FnMut(&Record) -> CliResult<bool>,
{
    let strategy = LimitedBufStrategy {
        double_until: 1 << 23,
        limit: max_mem,
    };
    match *format {
        InFormat::FASTA => {
            let mut rdr = seq_io::fasta::Reader::with_cap_and_strategy(rdr, cap, strategy);
                run_rdr!(rdr, func, rec, {fasta::FastaRecord::new(rec)})
        }
        InFormat::FASTQ => {
            let mut rdr = seq_io::fastq::Reader::with_cap_and_strategy(rdr, cap, strategy);
                run_rdr!(rdr, func, rec, {fastq::FastqRecord::new(rec)})
        }
        InFormat::CSV(ref delim, ref fields, has_header) => {
            let mut rdr = csv::CsvReader::new(rdr, *delim, fields, has_header)?;
            run_rdr!(rdr, func, rec, {rec})
        }
    }
}

// contains format specific code
// should be nicer once one generic function can be used instead of
// multiple functions generated by parallel_record_impl!()
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
    W: Fn(&Record, &mut D, &mut S) + Send + Sync,
    F: FnMut(&Record, &mut D, &mut S) -> CliResult<bool>,
{
    // not very nice, but saves some repetitition
    macro_rules! transform_result {
        ($res:expr) => {
            {
                match $res {
                    Ok(res) => if !res {
                        return Some(Ok(()));
                    },
                    Err(e) => return Some(Err(e))
                }
                None
            }
        }
    }

    let queue_len = n_threads as usize * 2;

    let out: CliResult<Option<CliResult<()>>> = match *format {
        InFormat::FASTA =>
            seq_io::parallel::parallel_fasta_init(
                n_threads, queue_len,
                || Ok::<_, seq_io::fasta::Error>(seq_io::fasta::Reader::new(rdr)),
                || record_data_init().map(|d| (d, None)),
                rset_data_init,
                |rec, &mut (ref mut d, ref mut delim), s| {
                    let rec = fasta::FastaRecord::new(rec);
                    work(&rec as &Record, d, s);
                    *delim = rec.delim();
                },
                |rec, &mut (ref mut d, delim), s| {
                    let rec = fasta::FastaRecord::new(rec);
                    if let Some(_d) = delim {
                        rec.set_delim(_d);
                    }
                    transform_result!(func(&rec, d, s))
                }
            ),
        InFormat::FASTQ =>
            seq_io::parallel::parallel_fastq_init(
                n_threads, queue_len,
                || Ok::<_, seq_io::fastq::Error>(seq_io::fastq::Reader::new(rdr)),
                || record_data_init().map(|d| (d, None)),
                rset_data_init,
                |rec, &mut (ref mut d, ref mut delim), s| {
                    let rec = fastq::FastqRecord::new(rec);
                    work(&rec as &Record, d, s);
                    *delim = rec.delim();
                },
                |rec, &mut (ref mut d, delim), s| {
                    let rec = fastq::FastqRecord::new(rec);
                    if let Some(_d) = delim {
                        rec.set_delim(_d);
                    }
                    transform_result!(func(&rec, d, s))
                }
            ),
        InFormat::CSV(ref delim, ref fields, has_header) =>
            parallel_csv::parallel_csv_init(
                n_threads, queue_len,
                || csv::CsvReader::new(rdr, *delim, fields, has_header),
                record_data_init,
                rset_data_init,
                |rec, d, s| work(&rec as &Record, d, s),
                |rec, d, s| transform_result!(func(&rec as &Record, d, s))
            )
    };
    match out? {
        Some(Err(e)) => Err(e),
        _ => Ok(()),
    }
}
