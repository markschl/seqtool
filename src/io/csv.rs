use std::io;

use crate::error::{CliError, CliResult};
use crate::helpers::DefaultHashMap as HashMap;
use csv;

use super::{MaybeModified, Record, RecordHeader, SeqReader};

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TextColumnSpec {
    Index(usize),
    Name(String),
}

pub type ColumnMapping = (String, TextColumnSpec);

// Reader

pub struct CsvReader<R: io::Read> {
    rdr: csv::Reader<R>,
    rec: CsvRecord,
}

impl<R: io::Read> CsvReader<R> {
    pub fn new(
        rdr: R,
        delim: u8,
        fields: &[(String, TextColumnSpec)],
        mut has_header: bool,
    ) -> CliResult<CsvReader<R>> {
        if fields.is_empty() {
            return fail!("At least one delimited text column must be defined");
        }

        // assume headers in case of any named (non-integer) column
        if fields
            .iter()
            .any(|(_, f)| matches!(f, TextColumnSpec::Name(_)))
        {
            has_header = true;
        }

        let mut builder = csv::ReaderBuilder::new();
        let mut rdr = builder
            .delimiter(delim)
            .quoting(false)
            .has_headers(has_header)
            .flexible(true)
            .from_reader(rdr);

        let header = if has_header {
            Some(rdr.headers()?)
        } else {
            None
        };

        // field -> column index
        let mut fieldmap: HashMap<&str, usize> = fields
            .iter()
            .map(|(field, col)| {
                let idx = match *col {
                    TextColumnSpec::Index(idx) => idx,
                    TextColumnSpec::Name(ref name) => {
                        if let Some(idx) = header.as_ref().unwrap().iter().position(|h| h == name) {
                            idx
                        } else {
                            return fail!("Did not find field '{}' in header.", name);
                        }
                    }
                };
                Ok((field.as_str(), idx))
            })
            .collect::<Result<_, CliError>>()?;

        Ok(CsvReader {
            rdr,
            rec: CsvRecord {
                data: csv::ByteRecord::new(),
                cols: Columns {
                    initialized: true, // needed because of Default impl (used in parallel mod)
                    id_col: fieldmap
                        .remove("id")
                        .ok_or("Id (id) column must be defined with CSV input")?,
                    desc_col: fieldmap.remove("desc"),
                    seq_col: fieldmap
                        .remove("seq")
                        .ok_or("Sequence (seq) column must be defined with CSV input")?,
                    qual_col: fieldmap.remove("qual"),
                    // other_cols: fieldmap.into_iter().collect(),
                },
            },
        })
    }

    pub fn next(&mut self) -> Option<CliResult<&dyn Record>> {
        if !try_opt!(self.rdr.read_byte_record(&mut self.rec.data)) {
            return None;
        }
        Some(Ok(&self.rec))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Columns {
    initialized: bool,
    id_col: usize,
    desc_col: Option<usize>,
    seq_col: usize,
    qual_col: Option<usize>,
    // TODO: allow reading other data
    // other_cols: Vec<(String, usize)>,
}

impl<R> SeqReader for CsvReader<R>
where
    R: io::Read,
{
    fn read_next(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>> {
        self.next().map(|r| r.and_then(|r| func(&r)))
    }
}

// method used by seq_io::parallel module
impl<R: io::Read> CsvReader<R> {
    //type Record = CsvRecord;
    pub fn read_record(&mut self, record: &mut CsvRecord) -> Option<io::Result<()>> {
        if !try_opt!(self.rdr.read_byte_record(&mut self.rec.data)) {
            return None;
        }
        if !record.cols.initialized {
            record.cols = self.rec.cols.clone();
        }
        Some(Ok(()))
    }
}

// Record

#[derive(Debug, Clone)]
pub struct CsvRecord {
    data: csv::ByteRecord,
    cols: Columns,
}

impl Default for CsvRecord {
    fn default() -> CsvRecord {
        CsvRecord {
            data: csv::ByteRecord::new(),
            cols: Columns::default(),
        }
    }
}

impl Record for CsvRecord {
    fn id(&self) -> &[u8] {
        self.data.get(self.cols.id_col).unwrap_or(b"")
    }

    fn desc(&self) -> Option<&[u8]> {
        self.cols.desc_col.and_then(|i| self.data.get(i))
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        (self.id(), self.desc())
    }

    fn current_header(&self) -> RecordHeader {
        let (id, desc) = self.id_desc();
        RecordHeader::IdDesc(
            MaybeModified::new(id, false),
            MaybeModified::new(desc, false),
        )
    }

    fn raw_seq(&self) -> &[u8] {
        self.data.get(self.cols.seq_col).unwrap_or(b"")
    }

    fn qual(&self) -> Option<&[u8]> {
        self.cols.qual_col.map(|i| self.data.get(i).unwrap_or(b""))
    }
}
