use std::io;
use std::convert::AsRef;
use std::collections::HashMap;
use std::borrow::ToOwned;

use csv;
use lib::util::match_fields;
use error::CliResult;

use super::*;

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
            .has_headers(has_header)
            .flexible(true)
            .from_reader(rdr);

        let mut fieldmap: HashMap<_, _> = if fields[0].len() == 1 {
            // id,desc,seq
            if fields.iter().any(|f| f.len() > 1) {
                return fail!(
                    "Inconsistent CSV column description. Either use colons everywhere or nowhere."
                );
            }
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

            let indices: Vec<usize> = match idx {
                Ok(i) => i,
                Err(_) => {
                    // need to look up the indices
                    if !has_header {
                        rdr.read_byte_record(&mut csv::ByteRecord::new())?;
                    }
                    let header: Vec<_> = rdr.headers()?.iter().collect();
                    match_fields(&columns, &header)
                        .map_err(|f| format!("Did not find '{}' in header.", f))?
                }
            };
            seq_names.into_iter().zip(indices).collect()
        };

        Ok(CsvReader {
            rdr: rdr,
            rec: CsvRecord {
                data: csv::ByteRecord::new(),
                cols: Columns {
                    initialized: true, // needed because of Default impl (used in parallel mod)
                    id_col: fieldmap
                        .remove("id")
                        .ok_or("Id column must be defined with CSV input")?,
                    desc_col: fieldmap.remove("desc"),
                    seq_col: fieldmap
                        .remove("seq")
                        .ok_or("Sequence column must be defined with CSV input")?,
                    qual_col: fieldmap.remove("qual"),
                    other_cols: fieldmap.into_iter().collect(),
                },
            },
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Columns {
    initialized: bool,
    id_col: usize,
    desc_col: Option<usize>,
    seq_col: usize,
    qual_col: Option<usize>,
    other_cols: Vec<(String, usize)>,
}

impl<R: io::Read> SeqReader for CsvReader<R> {
    //type Record = CsvRecord;
    fn next(&mut self) -> Option<CliResult<&Record>> {
        if !try_opt!(self.rdr.read_byte_record(&mut self.rec.data)) {
            return None;
        }
        Some(Ok(&self.rec))
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

    fn raw_seq(&self) -> &[u8] {
        self.data.get(self.cols.seq_col).unwrap_or(b"")
    }

    fn qual(&self) -> Option<&[u8]> {
        self.cols.qual_col.map(|i| self.data.get(i).unwrap_or(b""))
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        to.extend_from_slice(self.raw_seq())
    }
}

// Writer
