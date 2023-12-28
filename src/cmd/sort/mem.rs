use std::cmp::{max, min};
use std::io::{self, Read, Write};
use std::path::PathBuf;

use byteorder::{ReadBytesExt, LE};
use rkyv::{
    ser::{
        serializers::{AlignedSerializer, BufferScratch, CompositeSerializer},
        Serializer,
    },
    AlignedVec, Deserialize, Infallible,
};

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

    pub fn clear(&mut self) {
        self.records.clear();
        self.mem = 0;
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

    pub fn get_file_sorter(&mut self, tmp_dir: PathBuf) -> io::Result<FileSorter> {
        let mut other = MemSorter::new(self.reverse, self.max_mem);
        other.records = self.records.drain(..).collect();
        FileSorter::from_mem(other, tmp_dir)
    }

    pub fn serialize_sorted(&mut self, mut io_writer: impl Write) -> CliResult<usize> {
        self.sort();
        let mut out = AlignedVec::new();
        let mut scratch = AlignedVec::new();
        for item in &self.records {
            out.clear();
            let mut serializer = CompositeSerializer::new(
                AlignedSerializer::new(&mut out),
                BufferScratch::new(&mut scratch),
                Infallible,
            );
            serializer.serialize_value(item).unwrap();
            let buf = serializer.into_components().0.into_inner();
            io_writer.write_all(&buf.len().to_le_bytes())?;
            io_writer.write_all(buf)?;
        }
        Ok(self.records.len())
    }

    pub fn deserialize_item(
        mut io_reader: impl Read,
        buf: &mut Vec<u8>,
    ) -> CliResult<Option<Item>> {
        let len = match io_reader.read_u64::<LE>() {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            res => res?,
        };
        buf.clear();
        buf.resize(len as usize, 0);
        io_reader.read_exact(buf)?;
        let archived = rkyv::check_archived_root::<Item>(&buf[..]).unwrap();
        // TODO: unsafe appears to save ~ 25% of time, add feature for activating unsafe?
        // let archived = unsafe { rkyv::archived_root::<Item>(&buf[..]) };
        let item = archived.deserialize(&mut Infallible).unwrap();
        Ok(Some(item))
    }
}


