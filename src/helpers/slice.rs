use std::mem::replace;

pub fn split_text(text: &'_ [u8], sep: u8) -> SplitIter<'_> {
    SplitIter { sep, text }
}

pub struct SplitIter<'a> {
    sep: u8,
    text: &'a [u8],
}

impl<'a> Iterator for SplitIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pos) = memchr::memchr(self.sep, self.text) {
            let (t, rest) = self.text.split_at(pos);
            self.text = &rest[1..];
            return Some(t);
        }
        if self.text.is_empty() {
            return None;
        }
        Some(replace(&mut self.text, b""))
    }
}
