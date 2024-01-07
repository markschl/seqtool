use std::cmp::{max, min};
use std::io::{self, Write};
use std::path::PathBuf;

use fxhash::FxBuildHasher;
use indexmap::IndexMap;

use crate::cmd::shared::{
    sort_item::{item_size, Item},
    tmp_store::{TmpHandle, TmpWriter},
};
use crate::error::CliResult;
use crate::helpers::value::SimpleValue;

use super::FileDeduplicator;

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

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
