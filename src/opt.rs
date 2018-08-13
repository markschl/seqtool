use std::convert::AsRef;
use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use docopt;

use error::CliError;
use error::CliResult;
use io::input::*;
use io::output::{OutFormat, OutputKind, OutputOptions};
use io::{Compression, QualFormat};
use lib::bytesize::parse_bytesize;
use lib::inner_result::MapRes;
use lib::util;
use var;

pub struct Args(docopt::ArgvMap);

impl Args {
    pub fn new(usage: &str) -> Result<Args, docopt::Error> {
        let d = docopt::Docopt::new(usage)?.help(true).parse()?;

        Ok(Args(d))
    }

    pub fn thread_num(&self) -> CliResult<u32> {
        let n = self.get_str("--threads");
        let n = n
            .parse()
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
        } else if self.0.get_bool("--fq-illumina") {
            (Some("fastq-illumina"), None, None)
        } else if let Some(fields) = self.opt_str("--csv") {
            (Some("csv"), Some(","), Some(fields))
        } else if let Some(fields) = self.opt_str("--tsv") {
            (Some("tsv"), Some("\t"), Some(fields))
        } else {
            (None, None, None)
        };

        fmt = self.opt_str("--fmt").or(fmt);
        delim = self.opt_str("--delim").or(delim);
        let fields = fields.unwrap_or_else(|| self.0.get_str("--fields"));
        let header = self.0.get_bool("--header");
        let qfile = self.opt_str("--qual");
        let cap = parse_bytesize(self.get_str("--buf-cap"))?.floor() as usize;
        let max_mem = parse_bytesize(self.get_str("--max-mem"))?.floor() as usize;
        let threaded = self.get_bool("--read-thread");
        let thread_bufsize = self
            .opt_str("--read-tbufsize")
            .map_res(|s| parse_bytesize(s))?
            .map(|s| s as usize);

        let (arg_fmt, arg_compr) = fmt
            .map_res(|fmt| {
                let (fmt, compr) = parse_format_str(fmt)?;
                Ok::<_, CliError>((Some(fmt), Some(compr)))
            })?
            .unwrap_or((None, None));

        let input: Vec<_> = paths
            .into_iter()
            .map(|path| {
                let (kind, compression, fmt_str) = if path == "-" {
                    (
                        InputType::Stdin,
                        arg_compr.unwrap_or(Compression::None),
                        arg_fmt.clone().unwrap_or_else(|| "fasta".to_string()),
                    )
                } else {
                    let (path_fmt, path_compr) = path_info(&path);

                    (
                        InputType::File(PathBuf::from(&path)),
                        arg_compr.or(path_compr).unwrap_or(Compression::None),
                        arg_fmt
                            .clone()
                            .unwrap_or_else(|| path_fmt.unwrap_or("fasta").to_string()),
                    )
                };

                let format = InFormat::from_opts(&fmt_str, delim, fields, header, qfile)?;

                Ok(InputOptions {
                    kind,
                    format,
                    compression,
                    threaded,
                    thread_bufsize,
                    cap,
                    max_mem,
                })
            })
            .collect::<CliResult<_>>()?;

        if input.is_empty() {
            return fail!("Input is empty.");
        }

        Ok(input)
    }

    pub fn get_output_opts(&self, informat: Option<&InFormat>) -> CliResult<OutputOptions> {
        let (fmt, delim, fields) = if self.0.get_bool("--to-fa") {
            (Some("fasta"), None, None)
        } else if self.0.get_bool("--to-fq") {
            (Some("fastq"), None, None)
        } else if let Some(fields) = self.opt_str("--to-csv") {
            (Some("csv"), Some(","), Some(fields))
        } else if let Some(fields) = self.opt_str("--to-tsv") {
            (Some("tsv"), Some("\t"), Some(fields))
        } else {
            (None, None, None)
        };

        let wrap_fasta = if let Some(w) = self.opt_str("--wrap") {
            Some(
                w.parse()
                    .map_err(|_| format!("Invalid value for --wrap: '{}'", w))?,
            )
        } else {
            None
        };

        let path = self.0.get_str("--output");
        let threaded = self.get_bool("--write-thread");
        let attrs = self.parse_attrs()?;
        let wrap_fasta = wrap_fasta;
        let csv_delim = self.opt_str("--out-delim").or(delim);
        let csv_fields = fields.unwrap_or_else(|| self.0.get_str("--outfields"));
        let thread_bufsize = self
            .opt_str("--write-tbufsize")
            .map_res(|s| parse_bytesize(s))?
            .map(|s| s as usize);
        let compr_level = self.opt_value("--compr-level")?;
        let qfile = self.opt_str("--qual-out");

        let (arg_fmt, arg_compr) = self
            .opt_str("--to")
            .or(fmt)
            .map(|fmt| {
                let (fmt, compr) = parse_format_str(fmt)?;
                Ok::<_, CliError>((Some(fmt), Some(compr)))
            })
            .unwrap_or(Ok((None, None)))?;

        let arg_fmt = arg_fmt.as_ref().map(String::as_str);

        let (kind, compr, fmt_opts) = if path == "-" {
            (
                OutputKind::Stdout,
                arg_compr,
                arg_fmt.unwrap_or_else(|| informat.unwrap_or(&InFormat::FASTA).name()),
            )
        } else {
            let (fmt, compr) = path_info(&path);
            (
                OutputKind::File(PathBuf::from(&path)),
                arg_compr.or(compr),
                arg_fmt.unwrap_or_else(|| {
                    fmt.unwrap_or_else(|| informat.unwrap_or(&InFormat::FASTA).name())
                }),
            )
        };

        Ok(OutputOptions {
            kind: kind,
            format: OutFormat::from_opts(
                fmt_opts, &attrs, wrap_fasta, csv_delim, csv_fields, informat, qfile,
            )?,
            compression: compr.unwrap_or(Compression::None),
            compression_level: compr_level,
            threaded: threaded,
            thread_bufsize: thread_bufsize,
        })
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
                delim: self
                    .opt_string_or_env("--adelim", "ST_ATTR_DELIM")
                    .unwrap_or_else(|| " ".to_string()),
                value_delim: self
                    .opt_string_or_env("--aval-delim", "ST_ATTRVAL_DELIM")
                    .unwrap_or_else(|| "=".to_string()),
            },
            allow_missing: self.0.get_bool("--missing"),
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
            Some(&docopt::Value::Plain(Some(ref v))) => v
                .parse::<T>()
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

    let compr = match ext.to_ascii_lowercase().as_str() {
        "gz" | "gzip" => Some(Compression::GZIP),
        "bz2" | "bzip2" => Some(Compression::BZIP2),
        "lz4" => Some(Compression::LZ4),
        "zst" => Some(Compression::ZSTD),
        _ => None,
    };

    let stem = match path.file_stem() {
        Some(stem) => Path::new(stem),
        None => return (None, compr),
    };

    let path = if compr.is_some() { stem } else { path };

    let fmt = match path.extension().and_then(OsStr::to_str) {
        Some(ext) => match ext.to_ascii_lowercase().as_str() {
            "fastq" | "fq" => Some("fastq"),
            "fasta" | "fa" | "fna" | "fsa" => Some("fasta"),
            "csv" => Some("csv"),
            "tsv" | "txt" => Some("tsv"),
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

pub fn parse_format_str(string: &str) -> CliResult<(String, Compression)> {
    let string = string.to_ascii_lowercase();
    let parts: Vec<_> = string.split('.').collect();
    let (ext, compr) = if parts.len() == 1 {
        (parts[0].to_string(), Compression::None)
    } else {
        let compr = Compression::from_str(parts[1]).ok_or_else(|| {
            format!(
                "Unknown compression format: '{}'. Valid formats are gz, bz2, lz4, 7z",
                parts[1]
            )
        })?;
        (parts[0].to_string(), compr)
    };

    Ok((ext, compr))
}
