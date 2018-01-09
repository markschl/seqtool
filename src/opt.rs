use std::str::FromStr;
use std::path::PathBuf;
use std::ascii::AsciiExt;
use std::path::Path;
use std::convert::AsRef;
use std::ffi::OsStr;
use std::env;

use docopt;

use error::CliResult;
use error::CliError;
use var;
use io::input::*;
use io::output::{OutFormat, OutputKind, OutputOptions};
use io::Compression;
use lib::util;
use lib::bytesize::parse_bytesize;

pub struct Args(docopt::ArgvMap);

impl Args {
    pub fn new(usage: &str) -> Result<Args, docopt::Error> {
        let d = docopt::Docopt::new(usage)?.help(true).parse()?;

        Ok(Args(d))
    }

    pub fn thread_num(&self) -> CliResult<u32> {
        let n = self.get_str("--threads");
        let n = n.parse()
            .map_err(|_| format!("Invalid thread number: {}", n))?;
        if n == 0 {
            return fail!("The number of threads must be > 0");
        }
        Ok(n)
    }

    pub fn get_input_opts(&self) -> CliResult<Vec<InputOptions>> {
        let mut paths = self.get_vec("<input>");
        if paths.is_empty() {
            // default to stdin
            paths.push("-");
        }

        let (mut fmt, mut delim, fields) = if self.0.get_bool("--fa") {
            (Some("fasta"), None, None)
        } else if self.0.get_bool("--fq") {
            (Some("fastq"), None, None)
        } else if let Some(fields) = self.opt_str("--csv") {
            (Some("csv"), Some(","), Some(fields))
        } else if let Some(fields) = self.opt_str("--txt") {
            (Some("txt"), Some("\t"), Some(fields))
        } else {
            (None, None, None)
        };

        fmt = self.opt_str("--format").or(fmt);
        delim = self.opt_str("--delim").or(delim);
        let fields = fields.unwrap_or_else(|| self.0.get_str("--fields"));
        let header = self.0.get_bool("--header");

        let (arg_fmt, arg_compr) = match fmt {
            Some(fmt) => {
                let (fmt, compr) = parse_format_str(fmt)?;
                let fmt = InFormat::from_opts(&fmt, delim, fields, header)?;
                (Some(fmt), compr)
            }
            None => (None, None),
        };

        let mut input: Vec<InputOptions> = vec![];
        let cap = parse_bytesize(self.0.get_str("--buf-cap"))?.floor() as usize;
        let max_mem = parse_bytesize(self.0.get_str("--max-mem"))?.floor() as usize;
        let threaded_rdr = self.get_bool("--read-thread");
        let thread_bufsize = parse_bytesize(self.0.get_str("--read-tbufsize"))? as usize;

        for path in paths {
            let opts = if &*path == "-" {
                InputOptions {
                    kind: InputType::Stdin,
                    format: arg_fmt.clone().unwrap_or(InFormat::FASTA),
                    compression: arg_compr,
                    threaded: threaded_rdr,
                    thread_bufsize: thread_bufsize,
                    qfile: None,
                    cap: cap,
                    max_mem: max_mem,
                }
            } else {
                let (fmt, compr) = path_info(&path);

                InputOptions {
                    kind: InputType::File(PathBuf::from(&path)),
                    format: arg_fmt.clone().unwrap_or_else(|| {
                        fmt.map(|f| InFormat::from_opts(f, delim, fields, header).unwrap())
                            .unwrap_or(InFormat::FASTA)
                    }),
                    compression: arg_compr.or(compr),
                    threaded: threaded_rdr,
                    thread_bufsize: thread_bufsize,
                    qfile: None,
                    cap: cap,
                    max_mem: max_mem,
                }
            };

            input.push(opts);
        }

        if input.is_empty() {
            return fail!("Input is empty.");
        }

        Ok(input)
    }

    pub fn get_output_opts(&self, informat: Option<&InFormat>) -> CliResult<Option<OutputOptions>> {
        let (fmt, delim, fields) = if self.0.get_bool("--to-fa") {
            (Some("fasta"), None, None)
        } else if self.0.get_bool("--to-fq") {
            (Some("fastq"), None, None)
        } else if let Some(fields) = self.opt_str("--to-csv") {
            (Some("csv"), Some(","), Some(fields))
        } else if let Some(fields) = self.opt_str("--to-txt") {
            (Some("txt"), Some("\t"), Some(fields))
        } else {
            (None, None, None)
        };

        if self.0.get_bool("--no-output") {
            return Ok(None);
        }

        let wrap_fasta = if let Some(w) = self.opt_str("--wrap") {
            Some(w.parse()
                .map_err(|_| format!("Invalid value for --wrap: '{}'", w))?)
        } else {
            None
        };

        let path = self.0.get_str("--output");
        let threaded = self.get_bool("--write-thread");
        let attrs = self.parse_attrs()?;
        let wrap_fasta = wrap_fasta;
        let csv_delim = self.opt_str("--out-delim").or(delim);
        let csv_fields = fields.unwrap_or_else(|| self.0.get_str("--outfields"));
        let thread_bufsize = parse_bytesize(self.0.get_str("--write-tbufsize"))? as usize;

        let (arg_fmt, arg_compr) = self.opt_str("--outformat").or(fmt)
            .map(|fmt| {
                let (fmt, compr) = parse_format_str(fmt)?;
                Ok::<_, CliError>((Some(fmt), compr))
            })
            .unwrap_or(Ok((None, None)))?;

        let opts =
            if path == "-" {
                OutputOptions {
                    kind: OutputKind::Stdout,
                    format: get_outformat(
                        arg_fmt
                            .as_ref()
                            .map(String::as_str)
                            .unwrap_or_else(|| informat.unwrap_or(&InFormat::FASTA).name()),
                        &attrs,
                        wrap_fasta,
                        csv_delim,
                        csv_fields,
                    ).unwrap(),
                    compression: arg_compr,
                    threaded: threaded,
                    thread_bufsize: thread_bufsize,
                }
            } else {
                let (fmt, compr) = path_info(&path);

                OutputOptions {
                    kind: OutputKind::File(PathBuf::from(&path)),
                    format: get_outformat(
                        arg_fmt.as_ref().map(String::as_str).unwrap_or_else(|| {
                            fmt.unwrap_or_else(|| informat.unwrap_or(&InFormat::FASTA).name())
                        }),
                        &attrs,
                        wrap_fasta,
                        csv_delim,
                        csv_fields,
                    ).unwrap(),
                    compression: arg_compr.or(compr),
                    threaded: threaded,
                    thread_bufsize: thread_bufsize,
                }
            };

        Ok(Some(opts))

    }

    pub fn get_env_opts(&self) -> CliResult<var::VarOpts> {
        let id_col: usize = self.0.get_str("--id-col").parse()?;
        if id_col == 0 {
            return fail!("ID column cannot be zero!");
        }

        Ok(var::VarOpts {
            lists: self.get_vec("--list"),
            list_delim: self.0.get_str("--ldelim"),
            has_header: self.0.get_bool("--lheader"),
            unordered: self.0.get_bool("--unordered"),
            id_col: id_col - 1,
            attr_opts: var::AttrOpts {
                delim: self.opt_string_or_env("--adelim", "SEQTOOL_ATTR_DELIM")
                            .unwrap_or_else(|| " ".to_string()),
                value_delim: self.opt_string_or_env("--aval-delim", "SEQTOOL_ATTRVAL_DELIM")
                            .unwrap_or_else(|| "=".to_string()),
            },
            allow_missing: self.0.get_bool("--allow-missing"),
            var_help: self.0.get_bool("--help-vars"),
        })
    }

    fn parse_attrs(&self) -> CliResult<Vec<(String, String)>> {
        //let v: Vec<_> = self.get_vec("--attr").into_iter().map(parse_attr).collect();
        self.get_vec("--attr").into_iter().map(parse_attr).collect()
    }

    pub fn get_bool(&self, opt: &str) -> bool {
        self.0.get_bool(opt)
    }

    pub fn get_str(&self, opt: &str) -> &str {
        self.0.get_str(opt)
    }

    pub fn get_vec(&self, opt: &str) -> Vec<&str> {
        self.0.get_vec(opt)
    }

    pub fn opt_str(&self, opt: &str) -> Option<&str> {
        let val = self.get_str(opt);
        if val == "" {
            None
        } else {
            Some(val)
        }
    }

    pub fn opt_string_or_env(&self, opt: &str, env: &str) -> Option<String> {
        self.opt_str(opt)
            .map(|s| s.to_string())
            .or_else(|| env::var(env).ok())
    }

    pub fn value<T: FromStr>(&self, opt: &str) -> CliResult<T> {
        self.opt_value(opt).map(|o| o.unwrap())
    }

    pub fn opt_value<T: FromStr>(&self, opt: &str) -> CliResult<Option<T>> {
        match self.0.find(opt) {
            Some(&docopt::Value::Plain(Some(ref v))) => v.parse::<T>()
                .map(Some)
                .map_err(|_| CliError::Other(format!("Invalid value for {}: '{}'", opt, v))),
            _ => Ok(None),
        }
    }

    pub fn yes_no(&self, opt: &str) -> CliResult<Option<bool>> {
        if let Some(v) = self.opt_str(opt) {
            if v != "yes" && v != "no" {
                return fail!(format!("The value for {} must be 'yes' or 'no'.", opt));
            }
            Ok(Some(v == "yes"))
        } else {
            Ok(None)
        }
    }
}

pub fn path_info<P: AsRef<Path>>(path: &P) -> (Option<&'static str>, Option<Compression>) {
    let path = path.as_ref();
    let ext = match path.extension().and_then(OsStr::to_str) {
        Some(ext) => ext,
        None => return (None, None),
    };

    let compr = match ext.as_bytes() {
        b"gz" | b"gzip" => Some(Compression::GZIP),
        b"bz2" | b"bzip2" => Some(Compression::BZIP2),
        b"lz4" => Some(Compression::LZ4),
        _ => None,
    };

    let stem = match path.file_stem() {
        Some(stem) => Path::new(stem),
        None => return (None, compr),
    };

    let path = if compr.is_some() { stem } else { path };

    let fmt = match path.extension().and_then(OsStr::to_str) {
        Some(ext) => match ext.as_bytes() {
            b"fastq" | b"fq" => Some("fastq"),
            b"fasta" | b"fa" | b"fna" | b"fsa" => Some("fasta"),
            b"csv" => Some("csv"),
            b"txt" => Some("txt"),
            _ => {
                eprintln!("Unknown extension: '{}', assuming FASTA format", ext);
                None
            }
        },
        None => None,
    };

    (fmt, compr)
}

pub fn parse_attr(text: &str) -> CliResult<(String, String)> {
    let mut parts = text.splitn(2, '=');
    let name = parts.next().unwrap().to_string();
    let val = match parts.next() {
        Some(p) => p.to_string(),
        None => {
            return fail!(format!(
                "Invalid attribute: '{}'. Attributes need to be in the format: name=value",
                name
            ))
        }
    };
    Ok((name, val))
}

pub fn get_outformat(
    string: &str,
    attrs: &[(String, String)],
    wrap_fasta: Option<usize>,
    csv_delim: Option<&str>,
    csv_fields: &str,
) -> CliResult<OutFormat> {
    let csv_fields = csv_fields.split(',').map(|s| s.to_string()).collect();

    let format = match string {
        "fasta" => OutFormat::FASTA(attrs.to_owned(), wrap_fasta),
        "fastq" => OutFormat::FASTQ(attrs.to_owned()),
        //"fastq64" => OutFormat::FASTQ(attrs.to_owned()),
        "csv" => OutFormat::CSV(util::parse_delimiter(csv_delim.unwrap_or(","))?, csv_fields),
        "txt" => OutFormat::CSV(
            util::parse_delimiter(csv_delim.unwrap_or("\t"))?,
            csv_fields,
        ),
        _ => {
            return Err(CliError::Other(format!(
                "Unknown output format: '{}'",
                string
            )))
        }
    };

    Ok(format)
}

pub fn parse_format_str(string: &str) -> CliResult<(String, Option<Compression>)> {
    let string = string.to_ascii_lowercase();
    let parts: Vec<_> = string.split('.').collect();
    let (ext, compr) = if parts.len() == 1 {
        (parts[0].to_string(), None)
    } else {
        let compr = match parts[1] {
            "gz" => Compression::GZIP,
            "bz2" => Compression::BZIP2,
            "lz4" => Compression::LZ4,
            #[cfg(feature = "lzma")]
            "7z" | "xz" => Compression::LZMA,
            _ => {
                return Err(CliError::Other(format!(
                    "Unknown compression format: '{}'. Valid formats are gz, bz2, lz4, 7z",
                    parts[1]
                )))
            }
        };
        (parts[0].to_string(), Some(compr))
    };

    Ok((ext, compr))
}
