use std::cmp::{max, min};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::cmd::shared::tmp_store::{TmpHandle, TmpWriter};
use crate::error::CliResult;

use super::file::FileSorter;
use super::Item;

#[derive(Debug, Clone)]
pub struct MemSorter {
    records: Vec<Item>,
    reverse: bool,
    mem: usize,
    max_mem: usize,
}

impl MemSorter {
    pub fn new(reverse: bool, max_mem: usize) -> Self {
        Self {
            // we cannot know the exact length of the input, we just initialize
            // with capacity that should at least hold some records, while still
            // not using too much memory
            records: Vec::with_capacity(max(1, min(10000, max_mem / 400))),
            reverse,
            mem: 0,
            max_mem,
        }
    }

    pub fn add(&mut self, item: Item) -> bool {
        self.mem += item.size();
        self.records.push(item);
        self.mem < self.max_mem
    }

    fn sort(&mut self) {
        if !self.reverse {
            self.records.sort_by(|i1, i2| i1.key.cmp(&i2.key));
        } else {
            self.records.sort_by(|i1, i2| i2.key.cmp(&i1.key));
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn reverse(&self) -> bool {
        self.reverse
    }

    pub fn write_sorted(&mut self, io_writer: &mut dyn Write) -> CliResult<()> {
        self.sort();
        for item in &self.records {
            io_writer.write_all(&item.record)?;
        }
        Ok(())
    }

    pub fn get_file_sorter(
        &mut self,
        tmp_dir: PathBuf,
        file_limit: usize,
    ) -> io::Result<FileSorter> {
        let mut other = MemSorter::new(self.reverse, self.max_mem);
        other.records = self.records.drain(..).collect();
        FileSorter::from_mem(other, tmp_dir, file_limit)
    }

    pub fn serialize_sorted(
        &mut self,
        mut writer: TmpWriter<Item>,
    ) -> io::Result<(usize, TmpHandle<Item>)> {
        self.sort();
        for item in &self.records {
            writer.write(item)?;
        }
        let n = self.records.len();
        self.records.clear();
        self.mem = 0;
        writer.done().map(|h| (n, h))
    }
}
