use std::ffi::OsStr;
use std::hash::Hasher;
use std::path::Path;
use std::str;

use xxhash_rust::xxh3::xxh3_64 as hash_one;
use xxhash_rust::xxh3::Xxh3 as SeqHasher;

use self::GeneralVar::*;
use crate::error::CliResult;
use crate::helpers::{
    complement::reverse_complement,
    seqtype::{SeqType, SeqtypeHelper},
};
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
            var_info!(upper_seq => "The sequence in uppercase letters"),
            var_info!(lower_seq => "The sequence in lowercase letters"),
            var_info!(
                seqhash([ignorecase]) =>
                    "Calculates a hash value from the sequence using the XXH3 algorithm. \
                    A hash is a integer number representing the sequence. \
                    In very rare cases, different sequences may lead to the same hash value, \
                    but for instance using 'seqhash' as key for the 'unique' command \
                    (de-replication) speeds up the process and requires less memory, \
                    at a very small risk of wrongly recognizing two different sequences \
                    as duplicates. The numbers can be negative."
            ),
            var_info!(
                seqhash_rev([ignorecase]) =>
                    "The hash value of the reverse-complemented sequence"
            ),
            var_info!(
                seqhash_both([ignorecase]) =>
                    "The sum of the hashes from the forward and reverse sequences. \
                    The result is always the same irrespective of the sequence orientation, \
                    which is useful when de-replicating sequences with potentially different \
                    orientations."
            ),
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
            (
                "Removing records with duplicate sequences from the input",
                "st unique seq input.fasta",
            ),
            (
                "Removing duplicate records in a case-insensitive manner, recognizing both \
                forward and reverse orientations.",
                "st unique seqhash_both(false) input.fasta",
            ),
        ])
    }
}

#[derive(Clone, Debug, PartialEq)]
enum GeneralVar {
    Id,
    Desc,
    Seq,
    UpperSeq,
    LowerSeq,
    SeqHash { ignorecase: bool },
    SeqHashRev { ignorecase: bool },
    SeqHashBoth { ignorecase: bool },
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
    seq_cache: Vec<u8>,
    seqtype_helper: SeqtypeHelper,
}

impl GeneralVars {
    pub fn new(seqtype_hint: Option<SeqType>) -> GeneralVars {
        GeneralVars {
            vars: vec![],
            num: 0,
            path_info: PathInfo::default(),
            seq_cache: vec![],
            seqtype_helper: SeqtypeHelper::new(seqtype_hint),
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
            "upper_seq" => (VarType::Text, UpperSeq),
            "lower_seq" => (VarType::Text, LowerSeq),
            "seqhash" => {
                let ignorecase = func.opt_arg_as::<bool>(0).transpose()?.unwrap_or(false);
                (VarType::Int, SeqHash { ignorecase })
            }
            "seqhash_rev" => {
                let ignorecase = func.opt_arg_as::<bool>(0).transpose()?.unwrap_or(false);
                (VarType::Int, SeqHashRev { ignorecase })
            }
            "seqhash_both" => {
                let ignorecase = func.opt_arg_as::<bool>(0).transpose()?.unwrap_or(false);
                (VarType::Int, SeqHashBoth { ignorecase })
            }
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
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        self.num += 1;

        for (var, id) in &mut self.vars {
            let sym = symbols.get_mut(*id).inner_mut();
            match var {
                Id => sym.set_attr(RecordAttr::Id),
                Desc => sym.set_attr(RecordAttr::Desc),
                Seq => sym.set_attr(RecordAttr::Seq),
                UpperSeq | LowerSeq => {
                    self.seq_cache.clear();
                    record.write_seq(&mut self.seq_cache);
                    if *var == LowerSeq {
                        self.seq_cache.make_ascii_lowercase();
                    } else {
                        self.seq_cache.make_ascii_uppercase();
                    }
                    sym.set_text(&self.seq_cache);
                }
                SeqHash { ignorecase } => {
                    let hash = seqhash(record, &mut self.seq_cache, *ignorecase);
                    sym.set_int(hash as i64);
                }
                SeqHashRev { ignorecase } => {
                    let ty = self.seqtype_helper.get_or_guess(record)?;
                    let hash = seqhash_rev(record, &mut self.seq_cache, ty, *ignorecase)?;
                    sym.set_int(hash as i64);
                }
                SeqHashBoth { ignorecase } => {
                    let ty = self.seqtype_helper.get_or_guess(record)?;
                    let hash = seqhash_both(record, &mut self.seq_cache, ty, *ignorecase)?;
                    sym.set_int(hash as i64);
                }
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

fn seqhash(record: &dyn Record, seq_buf: &mut Vec<u8>, ignorecase: bool) -> u64 {
    let mut hasher = SeqHasher::default();
    for seq in record.seq_segments() {
        let seq = if ignorecase {
            seq_buf.clear();
            seq_buf.extend_from_slice(seq);
            seq_buf.make_ascii_uppercase();
            &*seq_buf
        } else {
            seq
        };
        hasher.write(seq);
    }
    hasher.finish()
}

fn seqhash_rev(
    record: &dyn Record,
    seq_buf: &mut Vec<u8>,
    seqtype: SeqType,
    ignorecase: bool,
) -> CliResult<u64> {
    reverse_complement(record.seq_segments().rev(), seq_buf, seqtype)?;
    if ignorecase {
        seq_buf.make_ascii_uppercase();
    }
    Ok(hash_one(&seq_buf))
}

fn seqhash_both(
    record: &dyn Record,
    seq_buf: &mut Vec<u8>,
    seqtype: SeqType,
    ignorecase: bool,
) -> CliResult<u64> {
    let hash1 = seqhash(record, seq_buf, ignorecase);
    let hash2 = seqhash_rev(record, seq_buf, seqtype, ignorecase)?;
    Ok(hash1.wrapping_add(hash2))
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
