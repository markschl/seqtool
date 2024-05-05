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
    input::{InputKind, InputOptions},
    output::OutputOptions,
    QualConverter, Record, RecordAttr,
};
use crate::var::{attr::Attributes, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};

use super::VarProvider;

variable_enum! {
    /// # Data from records and input files
    ///
    ///
    ///
    /// # Examples
    ///
    /// Adding the sequence number to the ID
    ///
    /// `st set -i {id}_{seq_num}`
    ///
    /// Counting the number of records per file in the input
    ///
    /// `st count -k filename *.fasta`
    ///
    /// Removing records with duplicate sequences from the input
    ///
    /// `st unique seq input.fasta`
    ///
    /// Removing duplicate records in a case-insensitive manner, recognizing both
    /// forward and reverse orientations
    ///
    /// `st unique Seqhash_both(false) input.fasta`
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
        /// sequences may lead to the same hash value, but for instance using 'Seqhash'
        /// as key for the 'unique' command (de-replication) speeds up the process and
        /// requires less memory, at a very small risk of wrongly recognizing two
        /// different sequences as duplicates. The numbers can be negative.
        Seqhash(Number) { ignorecase: bool = false },
        /// The hash value of the reverse-complemented sequence
        SeqhashRev(Number) { ignorecase: bool = false },
        /// The sum of the hashes from the forward and reverse sequences.
        /// The result is always the same irrespective of the sequence orientation,
        /// which is useful when de-replicating sequences with potentially different
        /// orientations.
        SeqhashBoth(Number) { ignorecase: bool = false },
        /// Sequence number (n-th sequence in the input), starting from 1.
        /// If multiple sequence files are supplied, numbering is simply continued.
        /// Note that the output order can vary with multithreaded processing.
        SeqNum(Number),
        /// Sequence index, starting from 0 (continued across all sequence files).
        /// Note that the output order can vary with multithreaded processing.
        SeqIdx(Number),
        /// Path to the current input file (or '-' if reading from STDIN)
        Path(Text),
        /// Name of the current input file with extension (or '-')
        Filename(Text),
        /// Name of the current input file without extension (or '-')
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
    idx: usize,
    path_info: PathInfo,
    seq_cache: Vec<u8>,
    seqtype_helper: SeqtypeHelper,
}

impl GeneralVars {
    pub fn new(seqtype_hint: Option<SeqType>) -> GeneralVars {
        GeneralVars {
            vars: VarStore::default(),
            idx: 0,
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
                SeqNum => sym.set_int((self.idx + 1) as i64),
                SeqIdx => sym.set_int(self.idx as i64),
                Path => sym.set_text(self.path_info.path.as_ref().unwrap()),
                Filename => sym.set_text(self.path_info.name.as_ref().unwrap()),
                Filestem => sym.set_text(self.path_info.stem.as_ref().unwrap()),
                Extension => sym.set_text(self.path_info.ext.as_ref().unwrap()),
                Dirname => sym.set_text(self.path_info.dir.as_ref().unwrap()),
                DefaultExt => sym.set_text(&self.path_info.out_ext),
            }
        }
        self.idx += 1;
        Ok(())
    }

    fn init_output(&mut self, out_opts: &OutputOptions) -> Result<(), String> {
        out_opts
            .format
            .default_ext()
            .as_bytes()
            .clone_into(&mut self.path_info.out_ext);
        Ok(())
    }

    fn init_input(&mut self, in_opts: &InputOptions) -> Result<(), String> {
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
    reverse_complement(record.seq_segments().rev(), seq_buf, seqtype)?;
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
