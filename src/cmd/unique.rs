use std::cmp::{max, min};
use std::env::temp_dir;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use fxhash::FxBuildHasher;
use indexmap::IndexMap;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{
    bytesize::parse_bytesize, heap_merge::HeapMerge, value::SimpleValue, vec::VecFactory,
};
use crate::var::varstring::VarString;

use super::shared::{
    key_var::KeyVars,
    sort_item::{item_size, Item},
    tmp_store::{TmpHandle, TmpStore, TmpWriter},
};

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

/// De-replicate records, returning only unique ones.
///
/// The order of the records is the same as in the input unless the memory limit
/// is exceeded, in which case temporary files are used and the records are
/// sorted by the unique key. Specify -s/--sorted to always sort the output.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct UniqueCommand {
    /// The key used to determine, which records are unique.
    /// If not specified, records are de-replicated by the sequence.
    /// The key can be a single variable/function
    /// such as 'id', or a composed string, e.g. '{id}_{desc}'.
    /// For each key, the *first* encountered record is returned, and all
    /// remaining ones with the same key are discarded.
    #[arg(short, long, default_value = "seq")]
    key: String,

    /// Interpret the key as a number instead of text.
    /// This may improve performance if the key is numeric, which could occur with
    /// header attributes or fields from associated lists with metadata.
    #[arg(short, long)]
    numeric: bool,

    /// Sort the output by key.
    /// Without this option, the records are in input order if the memory limit
    /// is *not* exceeded, but will be sorte by key otherwise.
    #[arg(short, long)]
    sort: bool,

    /// Maximum amount of memory to use for de-duplicating.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    max_mem: usize,

    /// Path to temporary directory (only if memory limit is exceeded)
    #[arg(long)]
    temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, default_value_t = 1000)]
    temp_file_limit: usize,

    /// Silence any warnings
    #[arg(short, long)]
    quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

/// Factor indicating the memory that is found empirically by memory profiling
/// and adjusts the calculated memory usage (based on size of items)
/// to obtain the correct total size, correcting for the extra memory used by
/// the hash map, Vec::sort() and possibly other allocations (TODO: investigate further)
static MEM_OVERHEAD: f32 = 1.4;

pub fn run(mut cfg: Config, args: &UniqueCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    let mut record_buf_factory = VecFactory::new();
    let mut key_buf = SimpleValue::Text(Vec::new());
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let (var_key, _) = cfg.build_vars(|b| VarString::var_or_composed(&args.key, b))?;
        let mut dedup = Deduplicator::new(max_mem);

        cfg.read(|record, ctx| {
            // assemble key
            let key = ctx.command_vars::<KeyVars, _>(|key_mod, symbols| {
                let key = var_key.get_simple(&mut key_buf, symbols, record, force_numeric)?;
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;
            // add formatted record to hash set:
            // first, look for the entry without any copying/allocation
            if !dedup.has_key(&key) {
                // if not present, format the record and create an owned key that
                // we can add to the hashmap
                let record_out =
                    record_buf_factory.fill_vec(|out| format_writer.write(&record, out, ctx))?;
                dedup.insert(
                    key.into_owned(),
                    record_out,
                    &tmp_path,
                    args.temp_file_limit,
                    args.quiet,
                )?;
            }
            Ok(true)
        })?;
        // write unique output
        dedup.write(io_writer, args.sort, args.quiet, verbose)
    })
}

/// Object handling the de-duplication, either in memory or using temporary files
#[derive(Debug)]
enum Deduplicator {
    Mem(MemDeduplicator),
    File(FileDeduplicator),
}

impl Deduplicator {
    fn new(max_mem: usize) -> Self {
        Self::Mem(MemDeduplicator::new(max_mem))
    }

    fn has_key(&self, key: &SimpleValue) -> bool {
        match self {
            Self::Mem(m) => m.has_key(key),
            Self::File(f) => f.has_key(key),
        }
    }

    fn insert(
        &mut self,
        key: SimpleValue,
        record: Vec<u8>,
        tmp_path: &Path,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => {
                if !m.insert(key, record) {
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, writing to temporary file(s). \
                            Consider raising the limit (-M/--max-mem) to speed up sorting. \
                            Use -q/--quiet to silence this message.",
                            m.len()
                        );
                    }
                    let mut f = m.get_file_sorter(tmp_path.to_owned(), file_limit)?;
                    f.write_to_file(quiet)?;
                    *self = Self::File(f);
                }
            }
            Self::File(f) => {
                f.insert(key, record, quiet)?;
            }
        }
        Ok(())
    }

    fn write(
        &mut self,
        io_writer: &mut dyn Write,
        force_sort: bool,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_records(io_writer, force_sort),
            Self::File(f) => f.write_records(io_writer, quiet, verbose),
        }
    }
}

/// Object handling the de-duplication in memory.
/// The memory used by the items is tracked, and `insert` returns `false`
/// if the memory limit is exceeded.
#[derive(Debug)]
pub struct MemDeduplicator {
    records: FxIndexMap<SimpleValue, Vec<u8>>,
    mem: usize,
    max_mem: usize,
}

impl MemDeduplicator {
    pub fn new(max_mem: usize) -> Self {
        let mut records = FxIndexMap::default();
        // we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        records.reserve(max(1, min(10000, max_mem / 400)));
        Self {
            records,
            mem: 0,
            max_mem,
        }
    }

    pub fn has_key(&self, key: &SimpleValue) -> bool {
        self.records.contains_key(key)
    }

    pub fn insert(&mut self, key: SimpleValue, record: Vec<u8>) -> bool {
        self.mem += item_size(&key, &record);
        self.records.insert(key, record);
        self.mem < self.max_mem
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn write_records(&mut self, io_writer: &mut dyn Write, sort: bool) -> CliResult<()> {
        if sort {
            let mut records: Vec<_> = self.records.iter().collect();
            records.sort_by_key(|(k, _)| *k);
            for (_, record) in records {
                io_writer.write_all(record)?;
            }
        } else {
            for record in self.records.values() {
                io_writer.write_all(record)?;
            }
        }
        Ok(())
    }

    /// Converts the MemDeduplicator into a FileDeduplicator.
    /// `self` is not consumed due to implementation difficulties, instead
    /// the hashmap is copied over to a new MemDuplicator
    pub fn get_file_sorter(
        &mut self,
        tmp_dir: PathBuf,
        file_limit: usize,
    ) -> io::Result<FileDeduplicator> {
        let mut other = MemDeduplicator::new(self.max_mem);
        other.records = self.records.drain(..).collect();
        FileDeduplicator::from_mem(other, tmp_dir, file_limit)
    }

    /// Writes all items to a binary format, sorted by key
    pub fn serialize_sorted(
        &mut self,
        mut writer: TmpWriter<Item>,
    ) -> io::Result<(usize, TmpHandle<Item>)> {
        let mut records: Vec<_> = self
            .records
            .drain(..)
            .map(|(k, r)| Item::new(k, r))
            .collect();
        records.sort();
        let n = records.len();
        for item in records {
            writer.write(&item)?;
        }
        self.mem = 0;
        writer.done().map(|h| (n, h))
    }
}

/// Object handling the de-duplication using temporary files.
/// The final unique records are obtained in the `write_records` method.
#[derive(Debug)]
pub struct FileDeduplicator {
    mem_dedup: MemDeduplicator,
    tmp_store: TmpStore,
    handles: Vec<TmpHandle<Item>>,
    n_written: usize,
}

impl FileDeduplicator {
    pub fn from_mem(
        mem_dedup: MemDeduplicator,
        tmp_dir: PathBuf,
        file_limit: usize,
    ) -> io::Result<Self> {
        Ok(Self {
            mem_dedup,
            handles: Vec::new(),
            tmp_store: TmpStore::new(tmp_dir, "st_unique_", file_limit)?,
            n_written: 0,
        })
    }

    pub fn has_key(&self, key: &SimpleValue) -> bool {
        self.mem_dedup.has_key(key)
    }

    pub fn insert(&mut self, key: SimpleValue, record: Vec<u8>, quiet: bool) -> CliResult<bool> {
        if !self.mem_dedup.insert(key, record) {
            self.write_to_file(quiet)?;
        }
        Ok(true)
    }

    pub fn write_to_file(&mut self, quiet: bool) -> CliResult<()> {
        let writer = self.tmp_store.writer(quiet)?;
        let (n, handle) = self.mem_dedup.serialize_sorted(writer)?;
        self.n_written += n;
        self.handles.push(handle);
        Ok(())
    }

    /// Writes the last remaining chunk in memory to a temporary file, then
    /// obtains the sorted output from individually sorted temporary files
    /// (using k-way merging).
    /// The items in each file are unique, but there can be duplicates across
    /// different files. Since the merged output is sorted, duplicates are
    /// easily removed from the final output.
    pub fn write_records(
        &mut self,
        io_writer: &mut dyn Write,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        // write last chunk of records
        self.write_to_file(quiet)?;

        if verbose {
            eprintln!(
                "De-duplicated {} records using {} temporary files ({:.1} records per file).",
                self.n_written,
                self.handles.len(),
                self.n_written as f64 / self.handles.len() as f64
            );
        }

        // readers for all sorted file chunks
        let mut readers = self
            .handles
            .iter_mut()
            .map(|handle| handle.reader())
            .collect::<Result<Vec<_>, _>>()?;

        // use k-way merging of sorted chunks with a min-heap to obtain
        // the final sorted output
        let mut prev_item = None;
        let kmerge = HeapMerge::new(readers.iter_mut().collect(), false)?;
        for item in kmerge {
            let item = item?;
            if prev_item.as_ref() != Some(&item) {
                io_writer.write_all(&item.record)?;
                prev_item = Some(item);
            }
        }
        // clean up
        for rdr in readers {
            rdr.done()?;
        }
        Ok(())
    }
}
