use std::str;
use std::ffi::OsStr;
use std::path::Path;

use io::Record;
use io::input::{InputOptions, InputType};
use io::output::OutputOptions;
use error::CliResult;
use var::*;
use self::BuiltinVar::*;

pub struct BuiltinHelp;

impl VarHelp for BuiltinHelp {
    fn name(&self) -> &'static str {
        "Standard variables without prefix"
    }
    fn usage(&self) -> &'static str {
        "<variable>"
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            ("id", "Record ID (in FASTA/FASTQ: string before space)"),
            ("desc", "Record description (everything after space)"),
            ("seq", "Record sequence"),
            ("num", "Sequence number starting with 1"),
            (
                "path",
                "Path to the current input file (or '-' if reading from STDIN)",
            ),
            (
                "filename",
                "Name of the current input file with extension (or '-')",
            ),
            ("filestem", "Name of the current input file without extension (or '-')"),
            ("extension", "Extension of the current input file."),
        ])
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Adding the sequence number to the ID ",
                "seqtool set -i {id}_{num}",
            ),
            (
                "Counting the number of records per file in the input",
                "seqtool count -k filename *.fasta",
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
enum BuiltinVar {
    Id,
    Desc,
    Seq,
    Num,
    InPath,
    InName,
    InStem,
    DefaultExt,
    Ext,
}

#[derive(Debug, Default)]
struct PathInfo {
    path: Option<Vec<u8>>,
    name: Option<Vec<u8>>,
    stem: Option<Vec<u8>>,
    ext: Option<Vec<u8>>,
    out_ext: Vec<u8>,
}

#[derive(Debug)]
pub struct BuiltinVars {
    vars: Vec<(BuiltinVar, usize)>,
    num: usize,
    path_info: PathInfo,
}

impl BuiltinVars {
    pub fn new() -> BuiltinVars {
        BuiltinVars {
            vars: vec![],
            num: 0,
            path_info: PathInfo::default(),
        }
    }
}

impl VarProvider for BuiltinVars {
    fn prefix(&self) -> Option<&str> {
        None
    }

    fn name(&self) -> &'static str {
        "builtin"
    }

    fn register_var(&mut self, name: &str, id: usize, _: &mut VarStore) -> CliResult<bool> {
        let var = match name {
            "id" => Id,
            "desc" => Desc,
            "seq" => Seq,
            "num" => Num,
            "path" => {
                self.path_info.path = Some(vec![]);
                InPath
            }
            "filename" => {
                self.path_info.name = Some(vec![]);
                InName
            }
            "filestem" => {
                self.path_info.stem = Some(vec![]);
                InStem
            }
            "extension" => {
                self.path_info.ext = Some(vec![]);
                Ext
            }
            "default_ext" => DefaultExt,
            _ => return Ok(false),
        };
        self.vars.push((var, id));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(&mut self, record: &Record, data: &mut Data) -> CliResult<()> {
        self.num += 1;

        for &(var, id) in &self.vars {
            match var {
                Id => data.symbols.set_text(id, record.id_bytes()),
                Desc => data.symbols
                    .set_text(id, record.desc_bytes().unwrap_or(b"")),
                Seq => {
                    let concatenated = data.symbols.mut_text(id);
                    for s in record.seq_segments() {
                        concatenated.extend_from_slice(s);
                    }
                }
                Num => data.symbols.set_int(id, self.num as i64),
                InPath => data.symbols
                    .set_text(id, self.path_info.path.as_ref().unwrap()),
                InName => data.symbols
                    .set_text(id, self.path_info.name.as_ref().unwrap()),
                InStem => data.symbols
                    .set_text(id, self.path_info.stem.as_ref().unwrap()),
                Ext => data.symbols
                    .set_text(id, self.path_info.ext.as_ref().unwrap()),
                DefaultExt => data.symbols.set_text(id, &self.path_info.out_ext),
            }
        }
        Ok(())
    }

    fn out_opts(&mut self, out_opts: &OutputOptions) -> CliResult<()> {
        self.path_info.out_ext = out_opts.format.default_ext().as_bytes().to_owned();
        Ok(())
    }

    fn new_input(&mut self, in_opts: &InputOptions) -> CliResult<()> {
        if let Some(ref mut path) = self.path_info.path {
            write_os_str(in_opts, path, |p| Some(p.as_os_str()))
        }
        if let Some(ref mut name) = self.path_info.name {
            write_os_str(in_opts, name, |p| p.file_name())
        }
        if let Some(ref mut stem) = self.path_info.stem {
            write_os_str(in_opts, stem, |p| p.file_stem())
        }
        if let Some(ref mut ext) = self.path_info.ext {
            write_os_str(in_opts, ext, |p| p.extension())
        }
        Ok(())
    }
}

fn write_os_str<F>(in_opts: &InputOptions, out: &mut Vec<u8>, func: F)
where
    F: FnOnce(&Path) -> Option<&OsStr>,
{
    out.clear();
    match in_opts.kind {
        InputType::Stdin => out.extend_from_slice(b"-"),
        InputType::File(ref p) => {
            let s = func(p.as_path());
            if let Some(s) = s {
                out.extend_from_slice(
                    s.to_str()
                        .map(|s| s.into())
                        .unwrap_or_else(|| s.to_string_lossy())
                        .as_bytes(),
                );
            }
        }
    }
}
