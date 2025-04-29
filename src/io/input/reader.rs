use crate::error::CliResult;
use crate::io::Record;

/// Trait for reading sequence records
pub trait SeqReader {
    /// Reads the next record and provides it in a closure
    /// The functions may return `false` to indicate that reading should stop.
    fn read_next(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>>;
}
