use std::io;
use std::fmt;

use csv::{self, ByteRecord, Reader, ReaderBuilder};
use fxhash::FxHashMap;

use error::{CliError, CliResult};
use var::*;
use io::Record;

pub struct ListHelp;

impl VarHelp for ListHelp {
    fn name(&self) -> &'static str {
        "Entries of associated lists."
    }
    fn usage(&self) -> &'static str {
        "l:<field>"
    }
    fn desc(&self) -> Option<&'static str> {
        Some(
            "Fields from associated lists. (-l argument). Specify either a column number \
            e.g. {l:4}, or a column name ({l:<fieldname>}) if there is a header. With
            multiple -l arguments, the lists can be selected in the same order using
            l:<field>, l2:<field>, l3:<field>, and so on.",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Extracting sequences with coordinates stored in a BED file",
                "seqtool trim -l coordinates.bed -0 {l:2}..{l:3} input.fa > output.fa",
            ),
        ])
    }
}

pub struct ListVars<R, H>
where
    R: io::Read,
    H: IdFinder<R>,
{
    prefix: String,
    rdr: Reader<R>,
    // used for reading data into it
    record: ByteRecord,
    // (var_id, col_idx)
    columns: Vec<(usize, usize)>,
    // specifies user choice and used as flag to indicate whether header has not yet been skipped
    has_header: bool,
    header: Option<FxHashMap<String, usize>>,
    handler: H,
    id_col: usize,
    allow_missing: bool,
}

impl<R, H> ListVars<R, H>
where
    R: io::Read,
    H: IdFinder<R>,
{
    pub fn new(
        num: usize,
        reader: R,
        handler: H,
        id_col: usize,
        delim: u8,
        has_header: bool,
        allow_missing: bool,
    ) -> ListVars<R, H> {
        let r = ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .from_reader(reader);
        let prefix = if num == 1 {
            "l".to_string()
        } else {
            format!("l{}", num)
        };
        ListVars {
            prefix: prefix,
            rdr: r,
            record: ByteRecord::new(),
            columns: vec![],
            has_header: has_header,
            header: None,
            handler: handler,
            id_col: id_col,
            allow_missing: allow_missing,
        }
    }
}

impl<R, H> VarProvider for ListVars<R, H>
where
    R: io::Read + Send,
    H: IdFinder<R> + Send,
{
    fn prefix(&self) -> Option<&str> {
        Some(&self.prefix)
    }
    fn name(&self) -> &'static str {
        "csv"
    }

    fn register_var(&mut self, name: &str, id: usize, _: &mut VarStore) -> CliResult<bool> {
        if !self.has_header {
            if let Ok(idx) = name.parse::<usize>() {
                if idx == 0 {
                    return fail!("Error in associated list: 0 is not a valid row index.");
                }
                if self.header.is_none() {
                    self.columns.push((id, idx - 1));
                    return Ok(true);
                }
            }
        }

        // look up column name
        self.has_header = true; // switch to header mode
        if self.header.is_none() {
            let map = self.rdr
                .headers()?
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), i))
                .collect();
            self.header = Some(map);
        }

        if let Some(i) = self.header.as_mut().unwrap().get(name) {
            self.columns.push((id, *i));
            return Ok(true);
        }
        Ok(false)
    }

    fn has_vars(&self) -> bool {
        !self.columns.is_empty()
    }

    fn set(&mut self, record: &Record, data: &mut Data) -> CliResult<()> {
        if self.has_header {
            self.rdr.read_byte_record(&mut self.record)?;
            self.has_header = false;
        }

        let id = record.id_bytes();

        match self.handler
            .find(self.id_col, id, &mut self.rdr, &mut self.record)
        {
            Err(_) if self.allow_missing => for &(var_id, _) in &self.columns {
                data.symbols.set_none(var_id);
            },
            Err(e) => return fail!(e),
            _ => {}
        }

        for &(var_id, col_idx) in &self.columns {
            if let Some(val) = self.record.get(col_idx) {
                data.symbols.set_text(var_id, val);
            } else if self.allow_missing {
                data.symbols.set_none(var_id);
            } else {
                return fail!(ListError::ColMissing(id.to_owned(), col_idx));
            }
        }

        Ok(())
    }
}

// stub
impl<R: io::Read, H: IdFinder<R>> fmt::Debug for ListVars<R, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ListVars {{ prefix: {} }}", self.prefix)
    }
}

pub trait IdFinder<R: io::Read> {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
    ) -> Result<(), ListError>;
}

pub struct SyncIds;

impl<R: io::Read> IdFinder<R> for SyncIds {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
    ) -> Result<(), ListError> {
        if !rdr.read_byte_record(rec)? {
            return Err(ListError::ListTooShort(id.to_owned()));
        }
        let row_id = rec.get(id_col)
            .ok_or_else(|| ListError::NoId(rdr.position().clone()))?;
        if row_id != id {
            return Err(ListError::IdMismatch(id.to_owned(), row_id.to_owned()));
        }
        Ok(())
    }
}

pub struct Unordered(FxHashMap<Vec<u8>, ByteRecord>);

impl Unordered {
    pub fn new() -> Unordered {
        Unordered(FxHashMap::default())
    }
}

impl<R: io::Read> IdFinder<R> for Unordered {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
    ) -> Result<(), ListError> {
        if let Some(found) = self.0.get(id) {
            rec.clone_from(found);
            return Ok(());
        }

        while rdr.read_byte_record(rec)? {
            let row_id = rec.get(id_col)
                .ok_or_else(|| ListError::NoId(rdr.position().clone()))?;

            self.0
                .entry(row_id.to_owned())
                .or_insert_with(|| rec.clone());

            if row_id == id {
                return Ok(());
            }
        }

        Err(ListError::EntryMissing(id.to_owned()))
    }
}

pub enum ListError {
    NoId(csv::Position),
    IdMismatch(Vec<u8>, Vec<u8>),
    ListTooShort(Vec<u8>),
    EntryMissing(Vec<u8>),
    ColMissing(Vec<u8>, usize),
    Csv(csv::Error),
}

impl From<csv::Error> for ListError {
    fn from(err: csv::Error) -> ListError {
        ListError::Csv(err)
    }
}

impl From<ListError> for CliError {
    fn from(err: ListError) -> CliError {
        let msg = match err {
            ListError::IdMismatch(ref list_id, ref seq_id) => format!(
                "ID mismatch: expected '{}' but found '{}'. Use -u/--unordered if sequences and \
                 lists are not in same order.",
                String::from_utf8_lossy(list_id),
                String::from_utf8_lossy(seq_id)
            ),
            ListError::ListTooShort(ref seq_id) => format!(
                "Associated list does not have enough entries, expected '{}'.",
                String::from_utf8_lossy(seq_id)
            ),
            ListError::EntryMissing(ref list_id) => format!(
                "ID '{}' not found in associated list. Use -m/--missing if you expect \
                missing entries.",
                String::from_utf8_lossy(list_id)
            ),
            ListError::NoId(ref pos) => format!(
                "ID Column not found in record no. {} at line {}",
                pos.record() + 1,
                pos.line()
            ),
            ListError::ColMissing(ref rec_id, idx) => format!(
                "Column no. {} not found in list entry for '{}'",
                idx + 1,
                String::from_utf8_lossy(rec_id)
            ),
            ListError::Csv(ref err) => format!("{}", err),
        };
        CliError::Other(msg)
    }
}
