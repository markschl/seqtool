use std::io::{self, Write};
use std::path::PathBuf;

use crate::cmd::shared::tmp_store::{Item, Key, TmpHandle, TmpStore};
use crate::error::CliResult;
use crate::helpers::{heap_merge::HeapMerge, vec_buf::VecFactory};

use super::{MapWriter, MemDeduplicator, Record, RecordMap, Records};

/// Object handling the de-duplication using temporary files.
/// The final unique records are obtained in the `write_records` method.
#[derive(Debug)]
pub struct FileDeduplicator {
    mem_dedup: MemDeduplicator,
    tmp_store: TmpStore,
    handles: Vec<TmpHandle<Item<Record>>>,
    has_placeholders: bool,
    n_written: usize,
    n_unique: usize,
}

impl FileDeduplicator {
    pub fn from_mut_records(
        records: &mut Records,
        tmp_dir: PathBuf,
        file_limit: usize,
        max_mem: usize,
        quiet: bool,
    ) -> CliResult<Self> {
        let mut tmp_store = TmpStore::new(tmp_dir, "st_unique", file_limit)?;
        // serialize the first chunk of data (either from KeySet or Records variants)
        let writer = tmp_store.writer(quiet)?;
        let (n_written, handle) = records.sort_serialize(writer)?;
        // then, create a new MemDeduplicator with the Records variant
        // since from now on, formatted records have to be stored
        let mut records: Records = records.clone();
        if matches!(records, Records::KeySet(..)) {
            records = Records::Records {
                records: RecordMap::default(),
                defer_writing: true,
                has_placeholders: false,
                sort: false,
                required_info: None,
            };
        }
        // we also need to extract the 'has_placeholders' flag
        let has_placeholders = match records {
            Records::Records {
                has_placeholders, ..
            } => has_placeholders,
            _ => unreachable!(),
        };
        // check defer_writing
        let defer_writing = match records {
            Records::Records { defer_writing, .. } => defer_writing,
            _ => unreachable!(),
        };
        if !defer_writing {
            return fail!(
                "The internal data store (used for --map-out) reached the memory limit \
                Please raise the limit with -M/--max-mem."
            );
        }
        Ok(Self {
            mem_dedup: MemDeduplicator::from_records(records, max_mem),
            handles: vec![handle],
            tmp_store,
            has_placeholders,
            n_written,
            n_unique: 0,
        })
    }

    pub fn add<'a, I, F>(
        &mut self,
        key: &Key,
        id_fn: I,
        vec_factory: &mut VecFactory,
        write_fn: F,
        quiet: bool,
    ) -> CliResult<bool>
    where
        I: Fn() -> &'a [u8],
        F: FnMut(&mut dyn io::Write) -> CliResult<()>,
    {
        if !self
            .mem_dedup
            .add(key, id_fn, vec_factory, write_fn, None)?
        {
            // Memory limit reached -> write to file and start new in-memory
            // de-duplication process
            self.write_to_file(quiet)?;
        }
        Ok(true)
    }

    pub fn write_to_file(&mut self, quiet: bool) -> CliResult<()> {
        let writer = self.tmp_store.writer(quiet)?;
        let (n, handle) = self.mem_dedup.sort_serialize(writer)?;
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
    pub fn write_records<M: io::Write>(
        &mut self,
        io_writer: &mut dyn Write,
        mut map_writer: Option<&mut MapWriter<M>>,
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
        let mut current_item: Option<Item<Record>> = None;
        let kmerge = HeapMerge::new(&mut readers, false)?;
        for new_item in kmerge {
            let new_item = new_item?;
            if let Some(current) = current_item.as_mut() {
                if current.key == new_item.key {
                    if let Some(info) = current.record.duplicate_info.as_mut() {
                        info.add_other(new_item.record.duplicate_info.unwrap());
                    }
                    continue;
                }
                current
                    .record
                    .write_deferred(self.has_placeholders, io_writer)?;
                if let Some(w) = map_writer.as_mut() {
                    w.write(
                        &current.key,
                        current.record.duplicate_info.as_ref().unwrap(),
                    )?;
                }
                self.n_unique += 1;
            }
            current_item = Some(new_item);
        }
        // last item (same code as in loop above)
        if let Some(current) = current_item.as_ref() {
            current
                .record
                .write_deferred(self.has_placeholders, io_writer)?;
            if let Some(w) = map_writer.as_mut() {
                w.write(
                    &current.key,
                    current.record.duplicate_info.as_ref().unwrap(),
                )?;
            }
            self.n_unique += 1;
        }
        // clean up
        for rdr in readers {
            rdr.done()?;
        }
        Ok(())
    }

    pub fn n_unique(&self) -> usize {
        self.n_unique
    }
}
