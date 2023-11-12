use std::collections::hash_map::Entry;
use std::fmt;
use std::io;

use csv::{self, ByteRecord, Reader, ReaderBuilder};
use fxhash::FxHashMap;

use crate::error::{CliError, CliResult};
use crate::io::Record;
use crate::var::*;

pub struct ListHelp;

impl VarHelp for ListHelp {
    fn name(&self) -> &'static str {
        "Entries of associated lists"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "list_col(fieldnum) or list_col(fieldname)",
            "Obtain a value from an associated list column.",
        )])
    }

    fn desc(&self) -> Option<&'static str> {
        Some(
            "Fields from associated lists. (`-l` argument). Specify either a column number \
            e.g. {list_col(4)}, or a column name {list_col(fieldname)} if there is a header. \
            With multiple -l arguments, the lists are selected by index in the order \
            in which they were specified, e.g. \
            `list_col(field)`, `list_col(2, field)`, `list_col(3, field)`, and so on.",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Extracting sequences with coordinates stored in a BED file",
                "st trim -l coordinates.bed -0 {list_col(2)}..{list_col(3)} input.fa > output.fa",
            ),
            (
                "Selecting only sequences present in an associated list",
                "st filter -uml selected_ids.txt 'list_col(1) != undefined' input.fa > output.fa",
            ),
        ])
    }
}

#[derive(Debug)]
enum VarType {
    Col(usize),
    Exists,
}

pub struct ListVars<R, H>
where
    R: io::Read,
    H: IdFinder<R>,
{
    list_num: usize,
    total_lists: usize,
    rdr: Reader<R>,
    // used for reading data into it
    record: ByteRecord,
    // (var_id, col_idx)
    vars: Vec<(usize, VarType)>,
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
        list_num: usize,
        total_lists: usize,
        reader: R,
        handler: H,
        delim: u8,
    ) -> ListVars<R, H> {
        let r = ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .from_reader(reader);
        ListVars {
            list_num,
            total_lists,
            rdr: r,
            record: ByteRecord::new(),
            vars: vec![],
            header: None,
            handler,
            id_col: 0,
            has_header: false,
            allow_missing: false,
        }
    }

    pub fn id_col(mut self, id_col: usize) -> Self {
        self.id_col = id_col;
        self
    }

    pub fn has_header(mut self, has_header: bool) -> Self {
        self.has_header = has_header;
        self
    }

    pub fn allow_missing(mut self, allow_missing: bool) -> Self {
        self.allow_missing = allow_missing;
        self
    }

    fn get_col_index(&mut self, col: &str) -> CliResult<usize> {
        if !self.has_header {
            // assuming column indices
            if let Ok(idx) = col.parse::<usize>() {
                if idx == 0 {
                    return fail!("Error in associated list: 0 is not a valid row index.");
                }
                if self.header.is_none() {
                    return Ok(idx - 1);
                }
            }
        }

        // switch to header mode
        // look up column name
        let col_name: String =
            ArgValue::from_str(col).ok_or_else(|| format!("Invalid list column: {}", col))?;
        self.has_header = true;
        if self.header.is_none() {
            let map = self
                .rdr
                .headers()?
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), i))
                .collect();
            self.header = Some(map);
        }

        if let Some(i) = self.header.as_mut().unwrap().get(&col_name) {
            return Ok(*i);
        }

        fail!(format!("Unknown list column: '{}'", col_name))
    }

    fn check_list_num(&self, list_num: usize) -> CliResult<bool> {
        if list_num > self.total_lists {
            return fail!(format!(
                "`list_col` requested list No. {}, but only {} lists were supplied with -l/--list",
                list_num, self.total_lists
            ));
        }
        if list_num != self.list_num {
            // another list, not the current one
            return Ok(false);
        }
        Ok(true)
    }
}

impl<R, H> VarProvider for ListVars<R, H>
where
    R: io::Read + 'static,
    H: IdFinder<R> + 'static,
{
    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<bool> {
        let var = match func.name.as_ref() {
            "has_entry" => {
                func.ensure_arg_range(0, 1)?;
                let list_num = func.arg_as::<usize>(0).transpose()?.unwrap_or(1);
                if !self.check_list_num(list_num)? {
                    return Ok(false);
                }
                VarType::Exists
            }
            "list_col" => {
                debug_assert!(self.list_num != 0);
                let (list_num, col) = match func.num_args() {
                    1 => (1, func.arg(0).unwrap()),
                    2 => (
                            func.arg_as::<i64>(0).unwrap()? as usize,
                            func.arg(1).unwrap()
                        ),
                    _ => return fail!("`list_col` accepts only 1 or 2 arguments: list_col(col) or list_col(n, col)")
                };
                if !self.check_list_num(list_num)? {
                    return Ok(false);
                }

                let i = self.get_col_index(col)?;
                VarType::Col(i)
            }
            _ => return Ok(false),
        };

        self.vars.push((b.symbol_id(), var));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(&mut self, record: &dyn Record, data: &mut MetaData) -> CliResult<()> {
        // read header record once (if present)
        if self.has_header {
            self.rdr.read_byte_record(&mut self.record)?;
            self.has_header = false;
        }

        // find the next record
        let id = record.id_bytes();
        let exists = self.handler.find(
            self.id_col,
            id,
            &mut self.rdr,
            &mut self.record,
            self.allow_missing,
        )?;

        for (var_id, var) in &self.vars {
            match var {
                VarType::Col(i) => {
                    let sym = data.symbols.get_mut(*var_id);
                    if let Some(text) = self.record.get(*i) {
                        sym.set_text(text);
                    } else {
                        if !self.allow_missing {
                            return fail!(ListError::ColMissing(id.to_owned(), *i));
                        }
                        sym.set_none();
                    }
                }
                VarType::Exists => {
                    data.symbols.get_mut(*var_id).set_bool(exists);
                }
            }
        }

        Ok(())
    }
}

// stub
impl<R: io::Read, H: IdFinder<R>> fmt::Debug for ListVars<R, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ListVars {{ list_num: {} }}", self.list_num)
    }
}

pub trait IdFinder<R: io::Read> {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
        allow_missing: bool,
    ) -> Result<bool, ListError>;
}

pub struct SyncIds;

impl<R: io::Read> IdFinder<R> for SyncIds {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
        _allow_missing: bool,
    ) -> Result<bool, ListError> {
        if !rdr.read_byte_record(rec)? {
            return Err(ListError::ListTooShort(id.to_owned()));
        }
        let row_id = rec
            .get(id_col)
            .ok_or_else(|| ListError::NoId(rdr.position().clone()))?;
        if row_id != id {
            return Err(ListError::IdMismatch(id.to_owned(), row_id.to_owned()));
        }
        Ok(true)
    }
}

pub struct Unordered {
    record_map: FxHashMap<Vec<u8>, ByteRecord>,
    dup_found: bool,
}

impl Unordered {
    pub fn new() -> Unordered {
        Unordered {
            record_map: FxHashMap::default(),
            dup_found: false,
        }
    }
}

impl<R: io::Read> IdFinder<R> for Unordered {
    fn find(
        &mut self,
        id_col: usize,
        id: &[u8],
        rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
        allow_missing: bool,
    ) -> Result<bool, ListError> {
        // first, check the record map for the given ID
        if let Some(found) = self.record_map.get(id) {
            rec.clone_from(found);
            return Ok(true);
        }

        // if not found, read until the ID is encountered
        while rdr.read_byte_record(rec)? {
            let row_id = rec
                .get(id_col)
                .ok_or_else(|| ListError::NoId(rdr.position().clone()))?;

            match self.record_map.entry(row_id.to_owned()) {
                Entry::Vacant(e) => {
                    e.insert(rec.clone());
                }
                Entry::Occupied(_) => {
                    if !self.dup_found {
                        eprintln!(
                            "Found duplicate IDs in associated list (first: {}). \
                            Only the first entry in the list is used.",
                            String::from_utf8_lossy(row_id)
                        );
                        self.dup_found = true;
                    }
                }
            }

            if row_id == id {
                return Ok(true);
            }
        }

        if allow_missing {
            return Ok(false);
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
