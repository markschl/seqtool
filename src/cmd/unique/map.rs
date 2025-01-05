use std::io;

use clap::ValueEnum;

use crate::cmd::shared::sort_item::Key;
use crate::helpers::write_list::{write_list, write_list_with};

use super::DuplicateInfo;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq)]
pub enum MapFormat {
    /// Sequence ID, reference record ID
    Long,
    /// Like `long`, but sets the reference record ID to `*` for the reference
    /// record itself instead of repeating the same ID twice.
    LongStar,
    /// Tab-delimited list of all duplicates, with the reference record ID first
    /// (e.g. corresponds to Swarm output format).
    Wide,
    /// Reference ID, comma-delimited list of duplicates including the reference ID
    ///  (corresponds to mothur `.names` file).
    WideComma,
    /// Like `wide`, but with the unique key in the first column and all duplicate
    /// IDs in the following columns.
    WideKey,
}

pub struct MapWriter<W: io::Write> {
    inner: W,
    format: MapFormat,
}

impl<W: io::Write> MapWriter<W> {
    pub fn new(inner: W, format: MapFormat) -> Self {
        Self { inner, format }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }

    pub fn write(&mut self, key: &Key, duplicates: &DuplicateInfo) -> io::Result<()> {
        let ids = match duplicates {
            DuplicateInfo::Ids(ids) => ids,
            _ => panic!(),
        };
        match self.format {
            MapFormat::Long | MapFormat::LongStar => {
                let mut first = true;
                for id in ids {
                    self.inner.write_all(id)?;
                    write!(self.inner, "\t")?;
                    if self.format == MapFormat::LongStar && first {
                        first = false;
                        self.inner.write_all(b"*")?;
                    } else {
                        self.inner.write_all(&ids[0])?;
                    }
                    writeln!(self.inner)?;
                }
            }
            MapFormat::Wide | MapFormat::WideKey => {
                if self.format == MapFormat::WideKey {
                    write_list_with(key.as_slice(), b"\t", &mut self.inner, |v, o| v.write(o))?;
                    write!(self.inner, "\t")?;
                }
                write_list(ids, b"\t", &mut self.inner)?;
                writeln!(self.inner)?;
            }
            MapFormat::WideComma => {
                self.inner.write_all(&ids[0])?;
                write!(self.inner, "\t")?;
                write_list(ids, b",", &mut self.inner)?;
                writeln!(self.inner)?;
            }
        }
        Ok(())
    }
}
