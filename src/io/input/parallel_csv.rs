use std::io;
use std::slice;

use seq_io::parallel;

use crate::error::{CliError, CliResult};

use super::csv::*;

const RECORSET_SIZE: usize = 100;

pub struct CsvRecordSet(Vec<CsvRecord>);

impl Default for CsvRecordSet {
    fn default() -> CsvRecordSet {
        CsvRecordSet(vec![CsvRecord::default(); RECORSET_SIZE])
    }
}

impl<'a> IntoIterator for &'a CsvRecordSet {
    type Item = &'a CsvRecord;
    type IntoIter = CsvRecordSetIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub type CsvRecordSetIter<'a> = slice::Iter<'a, CsvRecord>;

impl<R> parallel::Reader for CsvReader<R>
where
    R: io::Read,
{
    type DataSet = CsvRecordSet;
    type Err = CliError;
    fn fill_data(&mut self, rset: &mut CsvRecordSet) -> Option<CliResult<()>> {
        let mut n = 0;
        for rec in &mut rset.0 {
            if let Some(res) = self.read_record(rec) {
                try_opt!(res);
                n += 1;
            } else {
                return None;
            }
        }
        // last recordset smaller
        rset.0.truncate(n);
        Some(Ok(()))
    }
}

parallel_record_impl!(
    parallel_csv,
    parallel_csv_init,
    R,
    CsvReader<R>,
    CsvRecordSet,
    &CsvRecord,
    CliError
);
