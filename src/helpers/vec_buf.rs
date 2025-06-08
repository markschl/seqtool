/// A "factory" of vectors that determines the best initial capacity in a quite
/// simple (not very sophisticated) way.
/// These vectors are intended to be used as buffers, to which many rounds of writing
/// done (using `io::Write::write_all()`).
/// In each writing round, `write_fn` may issue many repeated `write_all()`, so the capacity
/// is not easy to manage.
/// The needed capacity is recalculated in regular intervals to make sure
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

    pub fn get<F, E>(&mut self, mut write_fn: F) -> Result<Vec<u8>, E>
    where
        F: FnMut(&mut Vec<u8>) -> Result<(), E>,
    {
        if self.counter >= 1000 {
            self.maxlen = self.minlen;
            self.counter = 0;
        }
        let mut v = Vec::with_capacity(self.maxlen);
        write_fn(&mut v)?;
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
