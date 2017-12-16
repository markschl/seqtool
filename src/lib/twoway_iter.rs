use twoway;

pub struct TwowayIter<'p, 't> {
    pattern: &'p [u8],
    text: &'t [u8],
    pos: usize,
}
impl<'p, 't> TwowayIter<'p, 't> {
    #[inline]
    pub fn new(text: &'t [u8], pattern: &'p [u8]) -> TwowayIter<'p, 't> {
        TwowayIter {
            text: text,
            pattern: pattern,
            pos: 0,
        }
    }
}

impl<'p, 't> Iterator for TwowayIter<'p, 't> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        twoway::find_bytes(self.text, self.pattern).map(|i| {
            self.text = self.text.split_at(i + 1).1;
            let found_position = self.pos + i;
            self.pos = found_position + 1;
            found_position
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twoway_iter() {
        let m: Vec<_> = TwowayIter::new(b"ABCDEFGABCDEFG", b"CD").collect();
        assert_eq!(&m, &[2, 9]);
    }
}
