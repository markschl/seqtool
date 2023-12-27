use crate::error::CliResult;

/// A "factory" of vectors that determines the best initial capacity in a quite
/// simple (not very sophisticated) way.
/// The capacity is recalculated in regular intervals to make sure
/// that the vectors do not use too much memory.
#[derive(Debug, Default)]
pub struct VecFactory {
    minlen: usize,
    maxlen: usize,
    counter: u16,
}

impl VecFactory {
    pub fn new() -> VecFactory {
        Self::default()
    }

    pub fn fill_vec<F>(&mut self, mut func: F) -> CliResult<Vec<u8>>
    where
        F: FnMut(&mut Vec<u8>) -> CliResult<()>,
    {
        if self.counter >= 1000 {
            self.maxlen = self.minlen;
            self.counter = 0;
        }
        let mut v = Vec::with_capacity(self.maxlen);
        func(&mut v)?;
        v.shrink_to_fit();
        if v.len() > self.maxlen {
            self.maxlen = v.len();
            self.counter += 1;
        } else if v.len() < self.minlen {
            self.minlen = v.len();
        }
        Ok(v)
    }
}
