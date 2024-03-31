use std::io::{self, Write};
use std::path::PathBuf;

use crate::cmd::shared::tmp_store::{TmpHandle, TmpStore};
use crate::error::CliResult;
use crate::helpers::heap_merge::HeapMerge;

use super::{Item, MemSorter};

#[derive(Debug)]
pub struct FileSorter {
    mem_sorter: MemSorter,
    tmp_store: TmpStore,
    handles: Vec<TmpHandle<Item<Box<[u8]>>>>,
    n_written: usize,
}

impl FileSorter {
    pub fn from_mem(
        mem_sorter: MemSorter,
        tmp_dir: PathBuf,
        file_limit: usize,
    ) -> io::Result<Self> {
        Ok(Self {
            mem_sorter,
            handles: Vec::new(),
            tmp_store: TmpStore::new(tmp_dir, "st_sort_", file_limit)?,
            n_written: 0,
        })
    }

    pub fn add(&mut self, item: Item<Box<[u8]>>, quiet: bool) -> CliResult<bool> {
        if !self.mem_sorter.add(item) {
            self.write_to_file(quiet)?;
        }
        Ok(true)
    }

    pub fn write_to_file(&mut self, quiet: bool) -> CliResult<()> {
        let writer = self.tmp_store.writer(quiet)?;
        let (n, handle) = self.mem_sorter.serialize_sorted(writer)?;
        self.n_written += n;
        self.handles.push(handle);
        Ok(())
    }

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
                "Sorted {} records using {} temporary files ({:.1} records per file).",
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
        let kmerge = HeapMerge::new(&mut readers, self.mem_sorter.reverse())?;
        for item in kmerge {
            io_writer.write_all(&item?.record)?;
        }
        // clean up
        for rdr in readers {
            rdr.done()?;
        }
        Ok(())
    }
}
