use std::io;
use std::path::PathBuf;

use deepsize::DeepSizeOf;
use indexmap::{IndexMap, IndexSet};
use rkyv::{Archive, Deserialize, Serialize};

use crate::cmd::shared::{
    sort_item::{Item, Key},
    tmp_store::{TmpHandle, TmpWriter},
};
use crate::error::CliResult;
use crate::helpers::{vec::VecFactory, DefaultBuildHasher as BuildHasher};

use super::{FileDeduplicator, MapWriter, Record, RequiredInformation};

// type RecordMap<K, V> = std::collections::HashMap<K, V, BuildHasher>;
// type RecordSet<V> = std::collections::HashSet<V, BuildHasher>;
pub type RecordMap<K, V> = IndexMap<K, V, BuildHasher>;
pub type RecordSet<V> = IndexSet<V, BuildHasher>;

#[derive(Archive, Deserialize, Serialize, DeepSizeOf, Debug, Clone, PartialEq)]
#[archive(compare(PartialEq), check_bytes)]
pub enum DuplicateInfo {
    Count(u64),
    Ids(Vec<Box<[u8]>>),
}

impl DuplicateInfo {
    pub fn new(required: RequiredInformation) -> Self {
        match required {
            RequiredInformation::Count => DuplicateInfo::Count(0),
            RequiredInformation::Ids => DuplicateInfo::Ids(Vec::with_capacity(1)),
        }
    }

    /// Register a new record (whose ID can be retrieved with `id_fn` if necessary).
    /// Returns the additional memory used by sequence IDs.
    /// TODO: capacity not tracked
    pub fn add_record<'a>(&mut self, id_fn: impl Fn() -> &'a [u8]) -> usize {
        match self {
            DuplicateInfo::Count(n) => {
                *n += 1;
                0
            }
            DuplicateInfo::Ids(i) => {
                let id = id_fn().to_owned().into_boxed_slice();
                let size = id.deep_size_of();
                i.push(id);
                size
            }
        }
    }

    pub fn add_other(&mut self, other: DuplicateInfo) {
        match (self, other) {
            (DuplicateInfo::Count(n), DuplicateInfo::Count(m)) => *n += m,
            (DuplicateInfo::Ids(i), DuplicateInfo::Ids(mut j)) => i.append(&mut j),
            _ => {
                panic!();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Records {
    // simple and memory-efficient mode, where only keys are collected and
    // unique records immediately written to output
    KeySet(RecordSet<Key>),
    // more elaborate mode, where information on duplicates and/or
    // formatted records are as well collected.
    Records {
        records: RecordMap<Key, Record>,
        // if true, collect formatted records and wait with writing until the end
        // (either because variables need to be replaced or because we need to sort by key)
        defer_writing: bool,
        // are there variable placeholders that should be replaced while writing
        // (only with deferred writing)
        has_placeholders: bool,
        // sort by key? (if true, deferred writing is used)
        sort: bool,
        // required information
        required_info: Option<RequiredInformation>,
    },
}

impl Records {
    fn new(
        has_placeholders: bool,
        sort: bool,
        required_info: Option<RequiredInformation>,
        capacity: usize,
    ) -> Self {
        let defer_writing = has_placeholders || sort;
        if required_info.is_some() || defer_writing {
            let mut records = RecordMap::default();
            records.reserve(capacity);
            Self::Records {
                records,
                defer_writing,
                has_placeholders,
                sort,
                required_info,
            }
        } else {
            let mut records = RecordSet::default();
            records.reserve(capacity);
            Self::KeySet(records)
        }
    }

    /// Checks for presence of a key and if not present, adds the key to the
    /// internal hash set or map and either writes the formatted record to the
    /// output (using `write_fn`) or keeps a formatted record for later.
    /// Returns the number of bytes by which the memory usage of the internal
    /// storage increased, or `None` if nothing was done.
    /// `output` can be `None` if `defer_writing` is true.
    pub fn add<'a, I, F>(
        &mut self,
        key: &Key,
        id_fn: I,
        vec_factory: &mut VecFactory,
        mut write_fn: F,
        output: Option<&mut dyn io::Write>,
    ) -> CliResult<usize>
    where
        I: Fn() -> &'a [u8],
        F: FnMut(&mut dyn io::Write) -> CliResult<()>,
    {
        match self {
            // simple case
            Self::KeySet(records) => {
                if records.insert(key.clone()) {
                    write_fn(&mut output.unwrap())?;
                    // dbg!(std::mem::size_of_val(key), key.deep_size_of());
                    return Ok(key.deep_size_of());
                }
            }
            // more complicated cases (needs more memory and time)
            Self::Records {
                records,
                defer_writing,
                required_info,
                ..
            } => {
                let mut n = 0;
                if let Some(rec) = records.get_mut(key) {
                    // record exists -> only update associated information
                    if let Some(info) = &mut rec.duplicate_info {
                        n += info.add_record(id_fn);
                    }
                } else {
                    // does not exist -> add new
                    let record = if *defer_writing {
                        let rec = vec_factory.get(|vec| write_fn(vec))?;
                        Some(rec)
                    } else {
                        write_fn(&mut output.unwrap())?;
                        None
                    };
                    let duplicate_info = required_info.map(|i| {
                        let mut info = DuplicateInfo::new(i);
                        info.add_record(id_fn);
                        info
                    });
                    let key = key.clone();
                    let rec = Record {
                        record,
                        duplicate_info,
                    };
                    n += key.deep_size_of() + rec.deep_size_of();
                    // dbg!(std::mem::size_of_val(&key), std::mem::size_of_val(&rec), n);
                    records.insert(key, rec);
                }
                return Ok(n);
            }
        }
        Ok(0)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::KeySet(records) => records.len(),
            Self::Records { records, .. } => records.len(),
        }
    }

    pub fn write_deferred<M: io::Write>(
        &mut self,
        io_writer: &mut dyn io::Write,
        mut map_writer: Option<&mut MapWriter<M>>,
    ) -> CliResult<()> {
        match self {
            Self::Records {
                records,
                has_placeholders,
                sort,
                ..
            } => {
                if *sort {
                    records.sort_keys();
                }
                for (key, rec) in records {
                    rec.write_deferred(*has_placeholders, io_writer)?;
                    if let Some(w) = map_writer.as_mut() {
                        w.write(key, rec.duplicate_info.as_ref().unwrap())?;
                    }
                }
            }
            // KeySet is not used if sorting is activated
            // (formatted records have to be stored)
            Self::KeySet(..) => {}
        }
        Ok(())
    }

    /// Writes all items to a binary format, sorted by key.
    /// Simultaneously removes all elements from Self.
    pub fn sort_serialize(
        &mut self,
        mut writer: TmpWriter<Item<Record>>,
    ) -> io::Result<(usize, TmpHandle<Item<Record>>)> {
        let n = match self {
            Records::Records {
                ref mut records, ..
            } => {
                records.sort_keys();
                let n = records.len();
                for (k, r) in records.drain(..) {
                    writer.write(&Item::new(k, r))?;
                }
                n
            }
            Records::KeySet(ref mut records) => {
                records.sort();
                let n = records.len();
                for k in records.drain(..) {
                    let r = Record {
                        record: None,
                        duplicate_info: None,
                    };
                    writer.write(&Item::new(k, r))?;
                }
                n
            }
        };
        writer.done().map(|h| (n, h))
    }
}

/// Object handling the de-duplication in memory.
/// The memory used by the items is tracked, and `insert` returns `false`
/// if the memory limit is exceeded.
#[derive(Debug)]
pub struct MemDeduplicator {
    records: Records,
    mem: usize,
    max_mem: usize,
}

impl MemDeduplicator {
    pub fn new(
        max_mem: usize,
        has_placeholders: bool,
        sort: bool,
        required_info: Option<RequiredInformation>,
    ) -> Self {
        // Capacity: we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        let cap = (max_mem / 400).clamp(1, 10000);
        let records = Records::new(has_placeholders, sort, required_info, cap);
        Self {
            records,
            mem: cap * if has_placeholders || sort { 72 } else { 24 },
            max_mem,
        }
    }

    pub fn from_records(records: Records, max_mem: usize) -> Self {
        Self {
            records,
            mem: 0,
            max_mem,
        }
    }

    // add a key/record and either directly write to output or keep the formatted record for later
    pub fn add<'a, I, F>(
        &mut self,
        key: &Key,
        id_fn: I,
        vec_factory: &mut VecFactory,
        write_fn: F,
        output: Option<&mut dyn io::Write>,
    ) -> CliResult<bool>
    where
        I: Fn() -> &'a [u8],
        F: FnMut(&mut dyn io::Write) -> CliResult<()>,
    {
        let size = self
            .records
            .add(key, id_fn, vec_factory, write_fn, output)?;
        self.mem += size;
        Ok(self.mem < self.max_mem)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Write records to output, which have been kept in memory, either
    /// because we need to sort them, or because we need to insert the
    /// content of the `n_duplicates` or `duplicate_ids` variables.
    pub fn write_deferred<M: io::Write>(
        &mut self,
        io_writer: &mut dyn io::Write,
        map_writer: Option<&mut MapWriter<M>>,
    ) -> CliResult<()> {
        self.records.write_deferred(io_writer, map_writer)
    }

    /// Obtains a FileDeduplicator from this MemDeduplicator.
    /// `self` is not consumed due to implementation difficulties, instead
    /// the hash map is copied over to a new MemDuplicator, removing all elements from 'self'.
    pub fn get_file_sorter(
        &mut self,
        tmp_dir: PathBuf,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<FileDeduplicator> {
        FileDeduplicator::from_mut_records(
            &mut self.records,
            tmp_dir,
            file_limit,
            self.max_mem,
            quiet,
        )
    }

    /// Writes all items to a binary format, sorted by key.
    /// Simultaneously removes all elements from the MemDeduplicator.
    pub fn sort_serialize(
        &mut self,
        writer: TmpWriter<Item<Record>>,
    ) -> io::Result<(usize, TmpHandle<Item<Record>>)> {
        self.mem = 0;
        self.records.sort_serialize(writer)
    }
}
