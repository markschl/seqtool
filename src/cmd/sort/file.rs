use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fs::{remove_file, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;

use tempdir::TempDir;

use crate::error::CliResult;

use super::mem::MemSorter;
use super::Item;

/// Warning limit for number of temporary files
const TEMP_FILE_WARN_LIMIT: usize = 50;

/// Wrapper type for items, which are sorted by key only
#[derive(Debug, Clone)]
struct ItemOrd {
    item: Item,
    reverse: bool,
    source: usize,
}

impl ItemOrd {
    fn new(item: Item, reverse: bool, source: usize) -> Self {
        Self {
            item,
            reverse,
            source,
        }
    }
}

impl PartialOrd for ItemOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ItemOrd {
    fn eq(&self, other: &Self) -> bool {
        self.item.key == other.item.key
    }
}

impl Eq for ItemOrd {}

impl Ord for ItemOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        if !self.reverse {
            self.item.key.cmp(&other.item.key)
        } else {
            other.item.key.cmp(&self.item.key)
        }
    }
}

#[derive(Debug)]
pub struct FileSorter {
    mem_sorter: MemSorter,
    files: Vec<PathBuf>,
    tmp_dir: TempDir,
    n_written: usize,
}

impl FileSorter {
    pub fn from_mem(mem_sorter: MemSorter, tmp_dir: PathBuf) -> io::Result<Self> {
        Ok(Self {
            mem_sorter,
            files: Vec::new(),
            tmp_dir: TempDir::new_in(tmp_dir, "st_sort_")?,
            n_written: 0,
        })
    }

    pub fn add(&mut self, item: Item, file_limit: usize, quiet: bool) -> CliResult<bool> {
        if !self.mem_sorter.add(item) {
            self.write_to_file(file_limit, quiet)?;
        }
        Ok(true)
    }

    pub fn write_to_file(&mut self, file_limit: usize, quiet: bool) -> CliResult<()> {
        if self.mem_sorter.len() > 0 && !quiet {
            if self.files.len() == TEMP_FILE_WARN_LIMIT {
                eprintln!(
                    "Warning: sequence sorting resulted in many temporary files ({}). \
                    Consider increasing the memory limit (-M/--max-mem). \
                    Supply -q/--quiet to silence this warning.",
                    TEMP_FILE_WARN_LIMIT
                )
            }
            if self.files.len() == file_limit {
                return fail!(
                    "Too many temporary files ({}) created by sort command. \
                    Try a higher memory limit (-M/--max-mem)",
                    file_limit
                );
            }
            let new_path = self
                .tmp_dir
                .path()
                .join(format!("st_sort_{}.tmp", self.files.len()));
            let mut bufwriter = BufWriter::new(File::create(&new_path)?);
            self.n_written += self.mem_sorter.serialize_sorted(&mut bufwriter)?;
            bufwriter.get_mut().sync_all()?;
            // let mut compr_writer = lz4::EncoderBuilder::new()
            //     .build(bufwriter)?;
            // self.n_written += self.mem_sorter.serialize_sorted(&mut compr_writer)?;
            // let (mut bufwriter, res) = compr_writer.finish();
            // res?;
            // bufwriter.get_mut().sync_all()?;

            // let wtr = File::create(&new_path)?;
            // let wtr = lz4::EncoderBuilder::new().build(wtr)?;
            // let (mut writer, res) = thread_io::write::writer_finish(
            //     1 << 22,
            //     4,
            //     wtr,
            //     |w| self.mem_sorter.serialize_sorted(w),
            //     |w| w.finish(),
            // )?.1;
            // writer.sync_all()?;
            self.mem_sorter.clear();
            self.files.push(new_path);
        }
        Ok(())
    }

    pub fn write_records(
        &mut self,
        io_writer: &mut dyn Write,
        file_limit: usize,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        // write last chunk of records
        self.write_to_file(file_limit, quiet)?;

        if verbose {
            eprintln!(
                "Sorted {} records using {} temporary files ({:.1}) records per file on average.",
                self.n_written,
                self.files.len(),
                self.n_written as f64 / self.files.len() as f64
            );
        }

        {
            // readers for all sorted file chunks
            let mut readers = self
                .files
                .iter_mut()
                .map(|path| Ok(BufReader::new(File::open(path)?)))
                // .map(|path| {
                //     let bufreader = BufReader::new(File::open(path)?);
                //     Ok(lz4::Decoder::new(bufreader)?)
                // })
                .collect::<CliResult<Vec<_>>>()?;

            // use k-way merging of sorted chunks with a min-heap to obtain
            // the final sorted output
            let mut buf = Vec::new();
            let mut heap = BinaryHeap::with_capacity(self.files.len());
            for (i, rdr) in readers.iter_mut().enumerate() {
                if let Some(item) = MemSorter::deserialize_item(rdr, &mut buf)? {
                    heap.push(Reverse(ItemOrd::new(item, self.mem_sorter.reverse(), i)));
                }
            }
            while let Some(top) = heap.pop() {
                if let Some(next_item) =
                    MemSorter::deserialize_item(&mut readers[top.0.source], &mut buf)?
                {
                    heap.push(Reverse(ItemOrd::new(
                        next_item,
                        self.mem_sorter.reverse(),
                        top.0.source,
                    )));
                }
                io_writer.write_all(&top.0.item.record)?;
            }
        }
        // clean up
        for path in self.files.drain(..) {
            remove_file(path)?;
        }
        Ok(())
    }
}
