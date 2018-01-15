use io::SeqAttr;

use memchr::memchr;

use lib::key_value;



#[derive(Debug, Clone)]
pub struct AttrPosition {
    pub start: usize,
    pub value_start: usize,
    pub end: usize,
}


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Action {
    Edit,
    Delete,
}

#[derive(Debug)]
pub struct Attrs {
    parser: Parser,
    id_actions: Vec<(usize, Action, AttrPosition)>,
    desc_actions: Vec<(usize, Action, AttrPosition)>,
    append_ids: Vec<usize>,
    attr_delim: u8,
    attr_value_delim: u8,
    // the distinction of ID and description makes handling of spaces somehow complicated
    adelim_is_space: bool,
    append_attr: SeqAttr,
}

impl Attrs {
    pub fn new(attr_delim: u8, attr_value_delim: u8, append_attr: SeqAttr) -> Attrs {
        Attrs {
            parser: Parser::new(attr_delim, attr_value_delim),
            id_actions: vec![],
            desc_actions: vec![],
            append_ids: vec![],
            attr_delim: attr_delim,
            attr_value_delim: attr_value_delim,
            adelim_is_space: attr_delim == b' ',
            append_attr: append_attr,
        }
    }

    // Not a "smart" function, names must be added in order of IDs (just supplied to ensure
    // consistency). Only used for importing attributes from VarStore, which assigns the IDs
    pub fn add_attr(&mut self, name: &str, id: usize, action: Option<Action>) {
        self.parser.register_attr(name, id, action);
    }

    #[inline]
    pub fn has_attrs(&self) -> bool {
        !self.parser.data().is_empty()
    }

    pub fn parse(&mut self, id: &[u8], desc: Option<&[u8]>) {
        self.parser.reset();

        if !self.adelim_is_space {
            self.parser.parse(id, SeqAttr::Id, false);
        }

        if let Some(d) = desc {
            self.parser.parse(d, SeqAttr::Desc, self.adelim_is_space);
        }

        // Distribute all positions according to ID/Desc and actions
        self.id_actions.clear();
        self.desc_actions.clear();
        self.append_ids.clear();
        for (id, ref d) in self.parser.data().iter().enumerate() {
            if let Some(a) = d.action {
                if let Some(&(seq_attr, ref pos)) = d.pos.as_ref() {
                    match seq_attr {
                        SeqAttr::Id => self.id_actions.push((id, a, pos.clone())),
                        SeqAttr::Desc => self.desc_actions.push((id, a, pos.clone())),
                        _ => panic!()
                    }
                } else if a != Action::Delete {
                    self.append_ids.push(id);
                }
            }
        }
    }

    pub fn compose<F>(
        &self,
        id: &[u8],
        desc: Option<&[u8]>,
        out_id: &mut Vec<u8>,
        out_desc: &mut Vec<u8>,
        mut push_fn: F,
    ) where
        F: FnMut(usize, &mut Vec<u8>),
    {
        out_id.clear();
        out_desc.clear();

        self._compose(id, &self.id_actions, out_id, &mut push_fn);

        if let Some(d) = desc {
            self._compose(d, &self.desc_actions, out_desc, &mut push_fn);
        }

        if self.append_attr == SeqAttr::Id {
            self.append_missing(out_id, &self.append_ids, true, &mut push_fn);

        } else if self.append_attr == SeqAttr::Desc {
            let delim_before = !(self.adelim_is_space && out_desc.is_empty());
            self.append_missing(out_desc, &self.append_ids, delim_before, &mut push_fn);
        }
    }

    fn _compose<F>(
        &self,
        text: &[u8],
        positions: &[(usize, Action, AttrPosition)],
        new_text: &mut Vec<u8>,
        mut push_fn: F,
    ) where
        F: FnMut(usize, &mut Vec<u8>),
    {
        let mut prev_end = 0;
        for &(id, action, ref pos) in positions {
            match action {
                Action::Edit => {
                    new_text.extend_from_slice(&text[prev_end..pos.value_start]);
                    push_fn(id, new_text);
                }
                Action::Delete => {
                    // remove the delimiter before if possible, but pos.start == 0 is also possible
                    let end = if pos.start > prev_end {
                        pos.start - 1
                    } else {
                        pos.start
                    };
                    new_text.extend_from_slice(&text[prev_end..end]);
                }
            }
            prev_end = pos.end;
        }
        new_text.extend_from_slice(&text[prev_end..]);
    }

    fn append_missing<F>(&self, new_text: &mut Vec<u8>, ids: &[usize], delim_before: bool, mut push_fn: F)
    where
        F: FnMut(usize, &mut Vec<u8>),
    {
        let mut delim_before = delim_before;
        for &id in ids {
            let d = self.parser.data().get(id).unwrap();
            debug_assert!(d.pos.is_none());
            if delim_before {
                new_text.push(self.attr_delim);
            } else {
                delim_before = true;
            }
            new_text.extend_from_slice(d.name.as_bytes());
            new_text.push(self.attr_value_delim);
            push_fn(id, new_text);
        }
    }

    pub fn get_value<'a>(&self, id: usize, id_text: &'a [u8], desc_text: Option<&'a [u8]>)
        -> Option<&'a [u8]>
    {
        self.parser.get_pos(id).and_then(|&(seq_attr, ref pos)| {
            let text = match seq_attr {
                SeqAttr::Id => id_text,
                SeqAttr::Desc => {
                    if let Some(d) = desc_text {
                        d
                    } else {
                        return None;
                    }
                }
                _ => panic!(),
            };
            Some(&text[pos.value_start..pos.end])
        })
    }
}


#[derive(Debug)]
struct AttrData {
    // attribute name, edit/delete action if requested (add_attr)
    pub name: String,
    pub action: Option<Action>,
    // Positional information, changes with each record.
    // (Id/Desc, (start of value, end), search id
    pub pos: Option<(SeqAttr, AttrPosition)>,
    search_id: usize
}


#[derive(Debug)]
struct Parser {
    data: Vec<AttrData>,
    search_id: usize,
    num_found: usize,
    delim: u8,
    value_delim: u8,
}

impl Parser {

    pub fn new(delim: u8, value_delim: u8) -> Parser {
        Parser {
            data: vec![],
            search_id: 1,
            num_found: 0,
            delim: delim,
            value_delim: value_delim,
        }
    }

    fn parse(&mut self, text: &[u8], seq_attr: SeqAttr, search_start: bool) {
        if self.all_found() {
            return;
        }
        if search_start && self.check_pos(text, 0, seq_attr) {
            return;
        }
        let mut text = text;
        let mut offset = 0;
        while let Some(p) = memchr(self.delim, text) {
            let p = p + 1;
            text = text.split_at(p).1;
            offset += p;
            if self.check_pos(text, offset, seq_attr) {
                break;
            }
        }
    }

    fn check_pos(&mut self, text: &[u8], offset: usize, seq_attr: SeqAttr) -> bool {
        let rv = key_value::parse(text, self.delim, self.value_delim);
        if let Some((key, vstart, end)) = rv {
            let pos = AttrPosition {
                start: offset,
                value_start: offset + vstart,
                end: offset + end,
            };
            self.set_attr_pos(key, seq_attr, pos);
            return self.all_found();
        }
        false
    }

    // Not a "smart" function, names must be added in order of IDs (just supplied to ensure
    // consistency). Only used for importing attributes from VarStore, which assigns the IDs
    pub fn register_attr(&mut self, name: &str, id: usize, action: Option<Action>) {
        assert!(id == self.data.len());
        self.data.insert(id, AttrData {
            name: name.to_string(),
            action: action,
            // initial values, will be replaced
            pos: None,
            search_id: 0,
        });
    }

    pub fn reset(&mut self) {
        self.search_id += 1;
        self.num_found = 0;
    }

    pub fn all_found(&self) -> bool {
        self.num_found >= self.data.len()
    }

    pub fn set_attr_pos(&mut self, name: &[u8], attr: SeqAttr, pos: AttrPosition) -> Option<&AttrData> {
        for d in &mut self.data {
            if name == d.name.as_bytes() {
                if d.search_id != self.search_id { // position was not yet found in this round
                    self.num_found += 1;
                    d.pos = Some((attr, pos));
                    d.search_id = self.search_id;
                    return Some(&*d);
                } else {
                    d.pos = None;
                }
            }
        }
        None
    }

    pub fn get_pos(&self, id: usize) -> Option<&(SeqAttr, AttrPosition)> {
        self.data
            .get(id)
            .and_then(|d| d.pos.as_ref())
    }

    #[inline]
    pub fn data(&self) -> &[AttrData] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desc_parser() {
        let mut out_id = vec![];
        let mut out_desc = vec![];

        let id = b"id";
        let desc = Some(&b"desc a=0 b=1"[..]);

        let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
        a.add_attr("a", 0, None);
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_desc, b"desc a=0 b=val");

        let desc = Some(&b"desc a=0 c=1"[..]);
        let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
        a.add_attr("a", 0, None);
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_desc, b"desc a=0 c=1 b=val");

        let desc = Some(&b"desc a=0 b=1"[..]);
        let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Delete));
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_desc, b"desc b=val");
    }

    #[test]
    fn delim() {
        let mut out_id = vec![];
        let mut out_desc = vec![];
        let id = b"id;a=0";
        let desc = Some(&b"desc a:1"[..]);

        let mut a = Attrs::new(b';', b'=', SeqAttr::Id);
        a.add_attr("a", 0, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_id, b"id;a=val");
        assert_eq!(&out_desc, b"desc a:1");

        let mut a = Attrs::new(b' ', b':', SeqAttr::Id);
        a.add_attr("a", 0, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_id, b"id;a=0");
        assert_eq!(&out_desc, b"desc a:val");
    }

    #[test]
    fn missing() {
        let mut out_id = vec![];
        let mut out_desc = vec![];
        let id = b"id";
        let desc = Some(&b"desc a=0 c=2"[..]);

        let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Edit));
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_desc, b"desc a=val c=2 b=val");

        let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Delete));
        a.add_attr("b", 1, Some(Action::Delete));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
        });
        assert_eq!(&out_desc, b"desc c=2");
    }

    // #[bench]
    // fn bench_attr_parser(b: &mut test::Bencher) {
    //     let mut a = Attrs::new(b' ', b'=', SeqAttr::Desc);
    //     a.add_attr("a", 0, None);
    //     a.add_attr("b", 1, Some(Action::Edit));
    //     let id = b"id";
    //     let desc = Some(&b"asdf a=0 b=1"[..]);
    //     b.iter(|| {
    //         a.parse(id, desc);
    //     });
    // }

}
