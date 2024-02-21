use std::cmp::{max, min};
use std::io::{self, Write};
use std::path::PathBuf;

use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use memchr::memmem;

use crate::cmd::shared::{
    sort_item::{item_size, Item},
    tmp_store::{TmpHandle, TmpWriter},
};
use crate::error::CliResult;
use crate::helpers::{util::replace_iter, value::SimpleValue};

use super::{FileDeduplicator, DUP_PLACEHOLDER};

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

/// Object handling the de-duplication in memory.
/// The memory used by the items is tracked, and `insert` returns `false`
/// if the memory limit is exceeded.
#[derive(Debug)]
pub struct MemDeduplicator {
    records: FxIndexMap<SimpleValue, (Vec<u8>, u64)>,
    mem: usize,
    max_mem: usize,
    count_duplicates: bool,
}

impl MemDeduplicator {
    pub fn new(max_mem: usize, count_duplicates: bool) -> Self {
        let mut records = FxIndexMap::default();
        // we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        records.reserve(max(1, min(10000, max_mem / 400)));
        Self {
            records,
            mem: 0,
            max_mem,
            count_duplicates,
        }
    }

    pub fn insert(
        &mut self,
        key: &SimpleValue,
        mut record_fn: impl FnMut() -> CliResult<Vec<u8>>,
    ) -> CliResult<bool> {
        if let Some((_, n)) = self.records.get_mut(key) {
            if self.count_duplicates {
                *n += 1;
            }
            Ok(true)
        } else {
            let record = record_fn()?;
            self.mem += item_size(key, &record);
            self.records.insert(key.clone(), (record, 1));
            Ok(self.mem < self.max_mem)
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn write_records(&mut self, io_writer: &mut dyn Write, sort: bool) -> CliResult<()> {
        if sort {
            let mut records: Vec<_> = self.records.iter().collect();
            records.sort_by_key(|(k, _)| *k);
            for (_, (record, n)) in records {
                self.write_record(record, *n, io_writer)?;
            }
        } else {
            for (record, n) in self.records.values() {
                self.write_record(record, *n, io_writer)?;
            }
        }
        Ok(())
    }

    fn write_record(&self, record: &[u8], size: u64, out: &mut dyn Write) -> CliResult<()> {
        if !self.count_duplicates {
            out.write_all(record)?;
        } else {
            replace_iter(
                record,
                |o| write!(o, "{}", size),
                memmem::find_iter(record, DUP_PLACEHOLDER)
                    .map(|start| (start, start + DUP_PLACEHOLDER.len())),
                out,
            )?;
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
    ) -> CliResult<FileDeduplicator> {
        if self.count_duplicates {
            return Err("Memory limit reached while de-duplicating records. \
                However, cannot switch to on-disk sorting because the 'n_duplicates' variable \
                is used, but its value can (currently) not be obtained with this procedure. \
                Consider raising the memory limit (-M/--max-mem) or not using 'n_duplicates'."
                .into());
        }
        let mut other = MemDeduplicator::new(self.max_mem, false);
        other.records = self.records.drain(..).collect();
        FileDeduplicator::from_mem(other, tmp_dir, file_limit).map_err(|e| e.into())
    }

    /// Writes all items to a binary format, sorted by key
    pub fn serialize_sorted(
        &mut self,
        mut writer: TmpWriter<Item>,
    ) -> io::Result<(usize, TmpHandle<Item>)> {
        let mut records: Vec<_> = self
            .records
            .drain(..)
            .map(|(k, (r, _))| Item::new(k, r))
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
