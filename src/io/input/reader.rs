use crate::error::CliResult;
use crate::io::Record;

/// Trait for reading sequence records
pub trait SeqReader {
    /// Reads the next record and provides it in a closure.
    /// The closure may return `false` to indicate that reading should stop.
    /// Returns `Some(Ok(do_stop))` if a record was found, otherwise `None`
    fn read_next_conditional(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>>;

    /// Reads the next record and returns `true` if it was found.
    /// There is no way the closure can signal back that the reading should stop.
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> CliResult<()>) -> CliResult<bool> {
        self.read_next_conditional(&mut |rec| func(rec).map(|_| true))
            .unwrap_or(Ok(false))
    }
}

impl<'a> SeqReader for Box<dyn SeqReader + 'a> {
    fn read_next_conditional(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>> {
        (**self).read_next_conditional(func)
    }

    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> CliResult<()>) -> CliResult<bool> {
        (**self).read_next(func)
    }
}
