use std::io::{self, Write};
use std::path::PathBuf;

use crate::cmd::shared::{
    sort_item::Item,
    tmp_store::{TmpHandle, TmpStore},
};
use crate::error::CliResult;
use crate::helpers::{heap_merge::HeapMerge, value::SimpleValue};

use super::MemDeduplicator;

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
