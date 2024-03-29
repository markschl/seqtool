use std::ffi::OsStr;
use std::path::Path;
use std::str;

use self::GeneralVar::*;
use crate::error::CliResult;
use crate::io::{
    input::{InputKind, InputOptions},
    output::OutputOptions,
    QualConverter, Record, RecordAttr,
};
use crate::var::{
    attr::Attributes,
    func::Func,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct GeneralHelp;

impl VarProviderInfo for GeneralHelp {
    fn name(&self) -> &'static str {
        "Data from records and input files"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(id => "Record ID (in FASTA/FASTQ: everything before first space)"),
            var_info!(desc => "Record description (everything after first space)"),
            var_info!(seq => "Record sequence"),
            var_info!(
                num =>
                    "Sequence number, starting with 1. Note that the output order can vary with \
                    multithreaded processing."
            ),
            var_info!(path => "Path to the current input file (or '-' if reading from STDIN)"),
            var_info!(filename => "Name of the current input file with extension (or '-')"),
            var_info!(filestem => "Name of the current input file without extension (or '-')"),
            var_info!(dirname => "Name of the base directory of the current file (or '')"),
            var_info!(default_ext =>
                "Default file extension for the configured output format \
                (e.g. 'fasta' or 'fastq')"
            ),
        ]
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Adding the sequence number to the ID ",
                "st set -i {id}_{num}",
            ),
            (
                "Counting the number of records per file in the input",
                "st count -k filename *.fasta",
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
enum GeneralVar {
    Id,
    Desc,
    Seq,
    Num,
    InPath,
    InName,
    InStem,
    DefaultExt,
    Ext,
    Dir,
}

#[derive(Debug, Default)]
struct PathInfo {
    path: Option<Vec<u8>>,
    name: Option<Vec<u8>>,
    stem: Option<Vec<u8>>,
    ext: Option<Vec<u8>>,
    dir: Option<Vec<u8>>,
    out_ext: Vec<u8>,
}

#[derive(Debug)]
pub struct GeneralVars {
    vars: Vec<(GeneralVar, usize)>,
    num: usize,
    path_info: PathInfo,
}

impl GeneralVars {
    pub fn new() -> GeneralVars {
        GeneralVars {
            vars: vec![],
            num: 0,
            path_info: PathInfo::default(),
        }
    }
}

impl VarProvider for GeneralVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &GeneralHelp
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let (vt, var) = match func.name.as_str() {
            "id" => (VarType::Attr, Id),
            "desc" => (VarType::Attr, Desc),
            "seq" => (VarType::Attr, Seq),
            "num" => (VarType::Int, Num),
            "path" => {
                self.path_info.path = Some(vec![]);
                (VarType::Text, InPath)
            }
            "filename" => {
                self.path_info.name = Some(vec![]);
                (VarType::Text, InName)
            }
            "filestem" => {
                self.path_info.stem = Some(vec![]);
                (VarType::Text, InStem)
            }
            "extension" => {
                self.path_info.ext = Some(vec![]);
                (VarType::Text, Ext)
            }
            "dirname" => {
                self.path_info.dir = Some(vec![]);
                (VarType::Text, Dir)
            }
            "default_ext" => (VarType::Text, DefaultExt),
            _ => unreachable!(), // shouldn't happen
        };
        self.vars.push((var, b.symbol_id()));
        Ok(Some(vt))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(
        &mut self,
        _record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        self.num += 1;

        for &(var, id) in &self.vars {
            let sym = symbols.get_mut(id).inner_mut();
            match var {
                Id => sym.set_attr(RecordAttr::Id),
                Desc => sym.set_attr(RecordAttr::Desc),
                Seq => sym.set_attr(RecordAttr::Seq),
                Num => sym.set_int(self.num as i64),
                InPath => sym.set_text(self.path_info.path.as_ref().unwrap()),
                InName => sym.set_text(self.path_info.name.as_ref().unwrap()),
                InStem => sym.set_text(self.path_info.stem.as_ref().unwrap()),
                Ext => sym.set_text(self.path_info.ext.as_ref().unwrap()),
                Dir => sym.set_text(self.path_info.dir.as_ref().unwrap()),
                DefaultExt => sym.set_text(&self.path_info.out_ext),
            }
        }
        Ok(())
    }

    fn init(&mut self, out_opts: &OutputOptions) -> CliResult<()> {
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
        InputKind::Stdin => out.extend_from_slice(b"-"),
        InputKind::File(ref p) => {
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
