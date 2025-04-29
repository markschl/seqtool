use std::cell::Cell;

use memchr::memchr;
use seq_io::policy::BufPolicy;

#[derive(Default, Clone, Debug)]
pub struct FastxHeaderParser {
    delim_pos: Cell<Option<Option<usize>>>,
}

impl FastxHeaderParser {
    // #[inline(always)]
    pub fn id_desc<'a>(&self, head: &'a [u8]) -> (&'a [u8], Option<&'a [u8]>) {
        if self.delim_pos.get().is_none() {
            self.delim_pos.set(Some(memchr(b' ', head)));
        }
        Self::_split_header(head, self.delim_pos.get().unwrap())
    }

    fn _split_header(head: &[u8], delim: Option<usize>) -> (&[u8], Option<&[u8]>) {
        if let Some(d) = delim {
            let (id, desc) = head.split_at(d);
            (id, Some(&desc[1..]))
        } else {
            (head, None)
        }
    }

    pub fn parsed_id_desc<'a>(&self, head: &'a [u8]) -> Option<(&'a [u8], Option<&'a [u8]>)> {
        self.delim_pos.get().map(|d| Self::_split_header(head, d))
    }

    pub fn delim_pos(&self) -> Option<Option<usize>> {
        self.delim_pos.get()
    }

    pub fn set_delim_pos(&self, delim_pos: Option<Option<usize>>) {
        self.delim_pos.set(delim_pos);
    }
}

#[derive(Clone)]
pub struct LimitedBuffer {
    pub double_until: usize,
    pub limit: usize,
}

impl BufPolicy for LimitedBuffer {
    fn grow_to(&mut self, current_size: usize) -> Option<usize> {
        if current_size < self.double_until {
            Some(current_size * 2)
        } else if current_size < self.limit {
            Some(current_size + self.double_until)
        } else {
            None
        }
    }
}
