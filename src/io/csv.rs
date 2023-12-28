use std::borrow::ToOwned;
use std::convert::AsRef;
use std::io;

use ::csv;
use fxhash::FxHashMap;

use super::*;
use crate::error::CliResult;
use crate::helpers::util::match_fields;

// Reader

pub struct CsvReader<R: io::Read> {
    rdr: csv::Reader<R>,
    rec: CsvRecord,
}

impl<R: io::Read> CsvReader<R> {
    pub fn new<I, S>(rdr: R, delim: u8, fields: I, has_header: bool) -> CliResult<CsvReader<R>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let fields: Vec<_> = fields
            .into_iter()
            .map(|f| {
                f.as_ref()
                    .splitn(2, ':')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        if fields.is_empty() {
            return fail!("At least one CSV field must be defined");
        }

        let mut builder = csv::ReaderBuilder::new();
        let mut rdr = builder
            .delimiter(delim)
            .quoting(false)
            .has_headers(has_header)
            .flexible(true)
            .from_reader(rdr);

        // check for consistency
        let n = fields[0].len();
        if fields.iter().any(|f| f.len() != n) {
            return fail!(
                "Inconsistent CSV column description. Either use colons everywhere or nowhere."
            );
        }

        let mut fieldmap: FxHashMap<_, _> = if n == 1 {
            // id,desc,seq
            fields
                .into_iter()
                .enumerate()
                .map(|(i, mut f)| (f.swap_remove(0), i))
                .collect()
        } else {
            // id:2,desc:6,seq:9
            // OR
            // id:id,seq:sequence,desc:description
            let (seq_names, columns): (Vec<String>, Vec<String>) = fields
                .into_iter()
                .map(|mut f| {
                    let f1 = f.remove(1);
                    (f.remove(0), f1)
                })
                .unzip();

            let idx: Result<Vec<_>, _> = columns.iter().map(|c| c.parse::<usize>()).collect();

            let indices: CliResult<Vec<usize>> = match idx {
                Ok(indices) => indices
                    .into_iter()
                    .map(|i| {
                        if i == 0 {
                            fail!("List column numbers should be > 1")
                        } else {
                            Ok(i - 1)
                        }
                    })
                    .collect(),
                Err(_) => {
                    // need to look up the indices
                    if !has_header {
                        rdr.read_byte_record(&mut csv::ByteRecord::new())?;
                    }
                    let header: Vec<_> = rdr.headers()?.iter().collect();
                    match_fields(&columns, &header)
                        .map_err(|f| format!("Did not find '{}' in header.", f).into())
                }
            };
            seq_names.into_iter().zip(indices?).collect()
        };

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

impl<R, O> SeqReader<O> for CsvReader<R>
where
    R: io::Read,
{
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>> {
        self.next().map(|r| r.map(|r| func(&r)))
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
    //type SeqSegments = OneSeqIter<'a>;
    fn id_bytes(&self) -> &[u8] {
        self.data.get(self.cols.id_col).unwrap_or(b"")
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.cols.desc_col.and_then(|i| self.data.get(i))
    }

    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (self.id_bytes(), self.desc_bytes())
    }

    fn get_header(&self) -> SeqHeader {
        let (id, desc) = self.id_desc_bytes();
        SeqHeader::IdDesc(id, desc)
    }

    fn raw_seq(&self) -> &[u8] {
        self.data.get(self.cols.seq_col).unwrap_or(b"")
    }

    fn has_seq_lines(&self) -> bool {
        false
    }

    fn qual(&self) -> Option<&[u8]> {
        self.cols.qual_col.map(|i| self.data.get(i).unwrap_or(b""))
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        to.extend_from_slice(self.raw_seq())
    }
}
