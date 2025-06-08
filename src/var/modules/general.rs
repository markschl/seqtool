use std::ffi::OsStr;
use std::hash::Hasher;
use std::path::Path;

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;
use xxhash_rust::xxh3::{xxh3_64 as hash_one, Xxh3 as Seqhasher};

use crate::helpers::{
    complement::reverse_complement,
    seqtype::{SeqType, SeqtypeHelper},
};
use crate::io::{
    input::{InputConfig, ReaderConfig},
    output::OutputConfig,
    IoKind, QualConverter, Record, RecordAttr,
};
use crate::var::{attr::Attributes, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

use super::VarProvider;

variable_enum! {
    /// # General properties of sequence records and input files
    ///
    ///
    /// # Examples
    ///
    /// Add the sequence number to the ID
    ///
    /// `st set -i {id}_{seq_num}`
    ///
    /// >A_1
    /// SEQUENCE
    /// >B_2
    /// SEQUENCE
    /// >C_3
    /// SEQUENCE
    /// (...)
    ///
    ///
    /// Count the number of records per file in the input
    ///
    /// `st count -k path *.fasta`
    ///
    /// file1.fasta	1224818
    /// file2.fasta	573
    /// file3.fasta	99186
    /// (...)
    ///
    ///
    /// Remove records with duplicate sequences from the input
    ///
    /// `st unique seq input.fasta`
    ///
    ///
    /// Remove duplicate records irrespective of the sequence orientation and
    /// whether letters are uppercase or lowercase
    ///
    /// `st unique 'seqhash_both(true)' input.fasta`
    GeneralVar {
        /// Record ID (in FASTA/FASTQ: everything before first space)
        Id(Text),
        /// Record description (everything after first space)
        Desc(Text),
        /// Record sequence
        Seq(Text),
        /// Record sequence in uppercase letters
        UpperSeq(Text),
        /// Record sequence in lowercase letters
        LowerSeq(Text),
        /// Calculates a hash value from the sequence using the XXH3 algorithm. A hash
        /// is a integer number representing the sequence. In very rare cases, different
        /// sequences may lead to the same hash value. Using 'seqhash' instead of 'seq'
        /// speeds up de-replication ('unique' command) and requires less memory,
        /// at a very small risk of wrongly recognizing two
        /// different sequences as duplicates.
        /// The returned numbers can be negative.
        Seqhash(Number) { ignorecase: bool = false },
        /// The hash value of the reverse-complemented sequence
        SeqhashRev(Number) { ignorecase: bool = false },
        /// The sum of the hashes from the forward and reverse sequences.
        /// The result is always the same irrespective of the sequence orientation,
        /// which is useful when de-replicating sequences with potentially different
        /// orientations. [side note: to be precise it is a *wrapping addition*
        /// to prevent integer overflow]
        SeqhashBoth(Number) { ignorecase: bool = false },
        /// Sequence number (n-th sequence in the input), starting from 1.
        /// The numbering continues across all provided sequence files unless `reset`
        /// is `true`, in which case the numbering re-starts from 1 for each new
        /// sequence file.
        ///
        /// Note that the output order can vary with multithreaded processing.
        SeqNum(Number) { reset: bool = false },
        /// Sequence index, starting from 0.
        ///
        /// The index is incremented across all provided sequence files unless `reset`
        /// is `true`, in which case the index is reset to 0 at the start of each
        /// new sequence file.
        ///
        /// Note that the output order can vary with multithreaded processing.
        SeqIdx(Number) { reset: bool = false },
        /// Path to the current input file (or '-' if reading from STDIN)
        Path(Text),
        /// Name of the current input file with extension (or '-')
        Filename(Text),
        /// Name of the current input file *without* extension (or '-')
        Filestem(Text),
        /// Extension of the current input file (or '')
        Extension(Text),
        /// Name of the base directory of the current file (or '')
        Dirname(Text),
        /// Default file extension for the configured output format
        /// (e.g. 'fasta' or 'fastq')
        DefaultExt(Text),
    }
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
    vars: VarStore<GeneralVar>,
    // index of file start, total index
    idx: Option<(usize, usize)>,
    path_info: PathInfo,
    seq_cache: Vec<u8>,
    seqtype_helper: SeqtypeHelper,
}

impl GeneralVars {
    pub fn new(seqtype_hint: Option<SeqType>) -> GeneralVars {
        GeneralVars {
            vars: VarStore::default(),
            idx: None,
            path_info: PathInfo::default(),
            seq_cache: vec![],
            seqtype_helper: SeqtypeHelper::new(seqtype_hint),
        }
    }
}

impl VarProvider for GeneralVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(GeneralVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(GeneralVar::from_func(name, args)?.map(|(var, out_type)| {
            use GeneralVar::*;
            match var {
                Path => self.path_info.path = Some(vec![]),
                Filename => self.path_info.name = Some(vec![]),
                Filestem => self.path_info.stem = Some(vec![]),
                Extension => self.path_info.ext = Some(vec![]),
                Dirname => self.path_info.dir = Some(vec![]),
                SeqIdx { .. } | SeqNum { .. } if self.idx.is_none() => {
                    self.idx = Some((0, 0));
                }
                _ => {}
            }
            let symbol_id: usize = builder.store_register(var, &mut self.vars);
            (symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set_record(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        for (id, var) in self.vars.iter() {
            let sym = symbols.get_mut(*id).inner_mut();
            use GeneralVar::*;
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
                Seqhash { ignorecase } => {
                    let hash = seqhash(record, &mut self.seq_cache, *ignorecase);
                    sym.set_int(hash as i64);
                }
                SeqhashRev { ignorecase } => {
                    let ty = self.seqtype_helper.get_or_guess(record)?;
                    let hash = seqhash_rev(record, &mut self.seq_cache, ty, *ignorecase)?;
                    sym.set_int(hash as i64);
                }
                SeqhashBoth { ignorecase } => {
                    let ty = self.seqtype_helper.get_or_guess(record)?;
                    let hash = seqhash_both(record, &mut self.seq_cache, ty, *ignorecase)?;
                    sym.set_int(hash as i64);
                }
                SeqNum { reset } => {
                    let (start_i, mut i) = self.idx.unwrap();
                    if *reset {
                        i -= start_i;
                    }
                    sym.set_int((i + 1) as i64);
                }
                SeqIdx { reset } => {
                    let (start_i, mut i) = self.idx.unwrap();
                    if *reset {
                        i -= start_i;
                    }
                    sym.set_int(i as i64);
                }
                Path => sym.set_text(self.path_info.path.as_ref().unwrap()),
                Filename => sym.set_text(self.path_info.name.as_ref().unwrap()),
                Filestem => sym.set_text(self.path_info.stem.as_ref().unwrap()),
                Extension => sym.set_text(self.path_info.ext.as_ref().unwrap()),
                Dirname => sym.set_text(self.path_info.dir.as_ref().unwrap()),
                DefaultExt => sym.set_text(&self.path_info.out_ext),
            }
        }
        if let Some((_, idx)) = self.idx.as_mut() {
            *idx += 1;
        }
        Ok(())
    }

    fn init_output(&mut self, cfg: &OutputConfig) -> Result<(), String> {
        cfg.format
            .default_ext()
            .as_bytes()
            .clone_into(&mut self.path_info.out_ext);
        Ok(())
    }

    fn init_input(&mut self, cfg: &InputConfig) -> Result<(), String> {
        if let Some(ref mut path) = self.path_info.path {
            write_os_str(&cfg.reader, path, |p| Some(p.as_os_str()))
        }
        if let Some(ref mut name) = self.path_info.name {
            write_os_str(&cfg.reader, name, |p| p.file_name())
        }
        if let Some(ref mut stem) = self.path_info.stem {
            write_os_str(&cfg.reader, stem, |p| p.file_stem())
        }
        if let Some(ref mut ext) = self.path_info.ext {
            write_os_str(&cfg.reader, ext, |p| p.extension())
        }
        if let Some((start_i, i)) = self.idx.as_mut() {
            *start_i = *i;
        }
        Ok(())
    }
}

fn seqhash(record: &dyn Record, seq_buf: &mut Vec<u8>, ignorecase: bool) -> u64 {
    let mut hasher = Seqhasher::default();
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
) -> Result<u64, String> {
    reverse_complement(record.seq_segments(), seq_buf, seqtype)?;
    if ignorecase {
        seq_buf.make_ascii_uppercase();
    }
    Ok(hash_one(seq_buf))
}

fn seqhash_both(
    record: &dyn Record,
    seq_buf: &mut Vec<u8>,
    seqtype: SeqType,
    ignorecase: bool,
) -> Result<u64, String> {
    let hash1 = seqhash(record, seq_buf, ignorecase);
    let hash2 = seqhash_rev(record, seq_buf, seqtype, ignorecase)?;
    Ok(hash1.wrapping_add(hash2))
}

fn write_os_str<F>(in_opts: &ReaderConfig, out: &mut Vec<u8>, func: F)
where
    F: FnOnce(&Path) -> Option<&OsStr>,
{
    out.clear();
    match in_opts.kind {
        IoKind::Stdio => out.extend_from_slice(b"-"),
        IoKind::File(ref p) => {
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
