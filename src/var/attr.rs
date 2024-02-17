use std::ops::Range;
use std::str::FromStr;

use memchr::memmem::{find, find_iter};
use winnow::combinator::{alt, rest, terminated};
use winnow::token::{take, take_until};
use winnow::{Located, PResult, Parser as _};

use crate::error::CliResult;
use crate::io::SeqAttr;

#[derive(Debug, Clone)]
pub struct AttrFormat {
    pub delim: Vec<u8>,
    pub value_delim: Vec<u8>,
}

impl AttrFormat {
    #[cfg(test)]
    pub fn new(delim: &[u8], value_delim: &[u8]) -> Self {
        Self {
            delim: delim.to_vec(),
            value_delim: value_delim.to_vec(),
        }
    }
}

impl FromStr for AttrFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s;
        let (delim, value_delim) =
            _parse_attr_format(&mut s).map_err(|_| format!("Invalid attribute format: {:?}", s))?;
        assert!(s.is_empty());
        Ok(AttrFormat {
            delim: delim.as_bytes().to_vec(),
            value_delim: value_delim.as_bytes().to_vec(),
        })
    }
}

pub fn _parse_attr_format<'a>(s: &mut &'a str) -> PResult<(&'a str, &'a str)> {
    let key = "key";
    let value = "value";
    let sep = take_until(1.., key).parse_next(s)?;
    take(key.len()).void().parse_next(s)?; // consume 'key'
    let value_sep = take_until(1.., value).parse_next(s)?;
    take(value.len()).void().parse_next(s)?; // consume 'value'
    Ok((sep, value_sep))
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Action {
    Edit,
    Delete,
}

#[derive(Debug)]
pub struct Attrs {
    parser: Parser,
    // (attr_id, name, action)
    actions: Vec<(usize, Action)>,
    // used to store current positions for each action
    _id_actions: Vec<(usize, Action, AttrPosition)>,
    _desc_actions: Vec<(usize, Action, AttrPosition)>,
    // if position was not found
    _append_ids: Vec<usize>,
    format: AttrFormat,
    // the distinction of ID and description makes handling of spaces somehow complicated
    adelim_is_space: bool,
    append_attr: SeqAttr,
}

impl Attrs {
    pub fn new(format: AttrFormat, append_attr: SeqAttr) -> Attrs {
        Attrs {
            parser: Parser::new(),
            actions: vec![],
            _id_actions: vec![],
            _desc_actions: vec![],
            _append_ids: vec![],
            adelim_is_space: format.delim == b" ",
            format,
            append_attr,
        }
    }

    // Not a "smart" function, names must be added in order of IDs (just supplied to ensure
    // consistency). Only used for importing attributes from VarStore, which assigns the IDs
    pub fn add_attr(&mut self, name: &str, id: usize, action: Option<Action>) {
        if let Some(a) = action {
            self.actions.push((id, a));
        }
        self.parser.register_attr(name, id);
    }

    #[inline]
    pub fn has_attrs(&self) -> bool {
        self.parser.has_attrs()
    }

    pub fn parse(&mut self, id: &[u8], desc: Option<&[u8]>) {
        self.parser.reset();

        if !self.adelim_is_space {
            self.parser.parse(id, &self.format, SeqAttr::Id, false);
        }

        if let Some(d) = desc {
            self.parser
                .parse(d, &self.format, SeqAttr::Desc, self.adelim_is_space);
        }

        // Distribute all positions according to ID/Desc and actions
        self._id_actions.clear();
        self._desc_actions.clear();
        self._append_ids.clear();
        for &(attr_id, action) in &self.actions {
            let (_, position) = self.parser.get(attr_id);
            if let Some(&(seq_attr, ref pos)) = position {
                match seq_attr {
                    SeqAttr::Id => self._id_actions.push((attr_id, action, pos.clone())),
                    SeqAttr::Desc => self._desc_actions.push((attr_id, action, pos.clone())),
                    _ => unimplemented!(),
                }
            } else if action != Action::Delete {
                self._append_ids.push(attr_id);
            }
        }
    }

    /// Composes attributes from input ID and description and writes them to
    /// output ID and description.
    /// The output vectors are cleared before composing.
    /// `push_fn` needs to be a custom supplied lookup function that translates
    /// attribute "ID"s to their corresponding values.
    pub fn compose<F>(
        &self,
        id: &[u8],
        desc: Option<&[u8]>,
        out_id: &mut Vec<u8>,
        out_desc: &mut Vec<u8>,
        mut push_fn: F,
    ) -> CliResult<()>
    where
        F: FnMut(usize, &mut Vec<u8>) -> CliResult<()>,
    {
        out_id.clear();
        out_desc.clear();

        self._compose(id, &self._id_actions, out_id, &mut push_fn)?;

        if let Some(d) = desc {
            self._compose(d, &self._desc_actions, out_desc, &mut push_fn)?;
        }

        if self.append_attr == SeqAttr::Id {
            self.append_missing(out_id, &self._append_ids, true, &mut push_fn)?;
        } else if self.append_attr == SeqAttr::Desc {
            let delim_before = !(self.adelim_is_space && out_desc.is_empty());
            self.append_missing(out_desc, &self._append_ids, delim_before, &mut push_fn)?;
        }
        Ok(())
    }

    fn _compose<F>(
        &self,
        text: &[u8],
        positions: &[(usize, Action, AttrPosition)],
        new_text: &mut Vec<u8>,
        mut push_fn: F,
    ) -> CliResult<()>
    where
        F: FnMut(usize, &mut Vec<u8>) -> CliResult<()>,
    {
        let mut prev_end = 0;
        for &(id, action, ref pos) in positions {
            match action {
                Action::Edit => {
                    new_text.extend_from_slice(&text[prev_end..pos.value_start]);
                    push_fn(id, new_text)?;
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
        Ok(())
    }

    fn append_missing<F>(
        &self,
        new_text: &mut Vec<u8>,
        ids: &[usize],
        delim_before: bool,
        mut push_fn: F,
    ) -> CliResult<()>
    where
        F: FnMut(usize, &mut Vec<u8>) -> CliResult<()>,
    {
        let mut delim_before = delim_before;
        for &attr_id in ids {
            let (attr_name, position) = self.parser.get(attr_id);
            debug_assert!(position.is_none());
            if delim_before {
                new_text.extend_from_slice(&self.format.delim);
            } else {
                delim_before = true;
            }
            new_text.extend_from_slice(attr_name.as_bytes());
            new_text.extend_from_slice(&self.format.value_delim);
            push_fn(attr_id, new_text)?;
        }
        Ok(())
    }

    pub fn has_value(&self, attr_id: usize) -> bool {
        let (_, position) = self.parser.get(attr_id);
        position.is_some()
    }

    pub fn get_value<'a>(
        &self,
        attr_id: usize,
        id_text: &'a [u8],
        desc_text: Option<&'a [u8]>,
    ) -> Option<&'a [u8]> {
        let (_, position) = self.parser.get(attr_id);
        position.and_then(|&(seq_attr, ref pos)| {
            let text = match seq_attr {
                SeqAttr::Id => id_text,
                SeqAttr::Desc => desc_text?,
                _ => panic!(),
            };
            Some(&text[pos.value_start..pos.end])
        })
    }
}

#[derive(Debug)]
struct AttrData {
    // used to know if the position is up-to date
    // (instead of resetting before each record)
    search_id: usize,
    // attribute name, edit/delete action if requested (add_attr)
    name: String,
    // Positional information, changes with each record.
    // (Id/Desc, (start of value, end), search id
    pos: (SeqAttr, AttrPosition),
}

impl AttrData {
    fn get_pos(&self, search_id: usize) -> Option<&(SeqAttr, AttrPosition)> {
        // TODO: replace search_id with Option<something>?
        if search_id == self.search_id {
            Some(&self.pos)
        } else {
            None
        }
    }

    // returns true if the position already exists for this search ID
    fn set_pos(&mut self, attr: SeqAttr, pos: AttrPosition, search_id: usize) -> bool {
        if search_id != self.search_id {
            // position was not yet found in this round
            self.pos = (attr, pos);
            self.search_id = search_id;
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Default)]
pub struct AttrPosition {
    pub start: usize,
    pub value_start: usize,
    pub end: usize,
}

/// Searches a key=value pair in a string (given format). Assumes that
/// 's' starts with the key.
fn parse_key_value<'a>(
    text: &'a [u8],
    offset: usize,
    format: &AttrFormat,
) -> Option<(&'a [u8], &'a [u8], AttrPosition)> {
    _parse_key_value(&mut Located::new(&text[offset..]), format)
        .ok()
        .map(|(k, v)| {
            (
                &text[offset + k.start..offset + k.end],
                &text[offset + v.start..offset + v.end],
                AttrPosition {
                    start: offset + k.start,
                    value_start: offset + v.start,
                    end: offset + v.end,
                },
            )
        })
}

/// winnow::Parser searching for key=value pairs
fn _parse_key_value(
    s: &mut Located<&'_ [u8]>,
    format: &AttrFormat,
) -> PResult<(Range<usize>, Range<usize>)> {
    (
        terminated(
            take_until(1.., format.value_delim.as_slice())
                .verify(|k: &[u8]| find(k, format.delim.as_slice()).is_none())
                .span(),
            take(format.value_delim.len()),
        ),
        alt((take_until(.., format.delim.as_slice()), rest)).span(),
    )
        .parse_next(s)
}

#[derive(Debug)]
struct Parser {
    data: Vec<AttrData>,
    search_id: usize,
    num_found: usize,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            data: vec![],
            search_id: 1,
            num_found: 0,
        }
    }

    fn parse(&mut self, text: &[u8], format: &AttrFormat, seq_attr: SeqAttr, check_start: bool) {
        if self.all_found() {
            return;
        }
        if check_start && self.find_key_value(text, format, 0, seq_attr) {
            return;
        }
        let l = format.delim.len();
        for pos in find_iter(text, &format.delim) {
            if self.find_key_value(text, format, pos + l, seq_attr) {
                break;
            }
        }
    }

    /// attempts at finding a key=value attribute at the start of the text
    fn find_key_value(
        &mut self,
        text: &[u8],
        format: &AttrFormat,
        offset: usize,
        seq_attr: SeqAttr,
    ) -> bool {
        if let Some((k, _v, pos)) = parse_key_value(text, offset, format) {
            self.set_attr_pos(k, seq_attr, pos);
            return self.all_found();
        }
        false
    }

    /// Requests an attribute to be searched in sequence headers.
    /// Not a "smart" function, attribute names must be added in order of IDs
    /// (which are just supplied to ensure consistency).
    pub fn register_attr(&mut self, name: &str, id: usize) {
        assert_eq!(id, self.data.len());
        self.data.insert(
            id,
            AttrData {
                name: name.to_string(),
                // initial values, will be replaced
                pos: (SeqAttr::Id, AttrPosition::default()),
                search_id: 0,
            },
        );
    }

    pub fn reset(&mut self) {
        self.search_id += 1;
        self.num_found = 0;
    }

    pub fn all_found(&self) -> bool {
        self.num_found >= self.data.len()
    }

    pub fn set_attr_pos(&mut self, name: &[u8], attr: SeqAttr, pos: AttrPosition) {
        // currently we do a linear search, since we don't assume that many
        // attributes are requested
        for d in &mut self.data {
            if name == d.name.as_bytes() {
                if !d.set_pos(attr, pos, self.search_id) {
                    self.num_found += 1;
                }
                break;
            }
        }
    }

    pub fn get(&self, attr_id: usize) -> (&str, Option<&(SeqAttr, AttrPosition)>) {
        let d = self.data.get(attr_id).unwrap();
        (&d.name, d.get_pos(self.search_id))
    }

    pub fn has_attrs(&self) -> bool {
        !self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value() {
        let kv = |s| {
            parse_key_value(s, 0, &AttrFormat::new(b" ", b"="))
                .map(|(k, _, p)| (k, p.value_start, p.end))
        };
        assert_eq!(kv(&b"k=v "[..]), Some((&b"k"[..], 2, 3)));
        assert_eq!(kv(&b"k=v"[..]), Some((&b"k"[..], 2, 3)));
        assert_eq!(kv(&b" k=v "[..]), None);
        assert_eq!(kv(&b"key=value "[..]), Some((&b"key"[..], 4, 9)));
        assert_eq!(kv(&b"ke y=value "[..]), None);
        assert_eq!(kv(&b"=v"[..]), None);
    }

    #[test]
    fn desc_parser() {
        let mut out_id = vec![];
        let mut out_desc = vec![];

        let id = b"id";
        let desc = Some(&b"desc a=0 b=1"[..]);

        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, None);
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_desc, b"desc a=0 b=val");

        let desc = Some(&b"desc a=0 c=1"[..]);
        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, None);
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_desc, b"desc a=0 c=1 b=val");

        let desc = Some(&b"desc a=0 b=1"[..]);
        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Delete));
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_desc, b"desc b=val");
    }

    #[test]
    fn delim() {
        let mut out_id = vec![];
        let mut out_desc = vec![];
        let id = b"id;a=0";
        let desc = Some(&b"desc a:1"[..]);

        let mut a = Attrs::new(AttrFormat::new(b";", b"="), SeqAttr::Id);
        a.add_attr("a", 0, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_id, b"id;a=val");
        assert_eq!(&out_desc, b"desc a:1");

        let mut a = Attrs::new(AttrFormat::new(b" ", b":"), SeqAttr::Id);
        a.add_attr("a", 0, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_id, b"id;a=0");
        assert_eq!(&out_desc, b"desc a:val");
    }

    #[test]
    fn missing() {
        let mut out_id = vec![];
        let mut out_desc = vec![];
        let id = b"id";
        let desc = Some(&b"desc a=0 c=2"[..]);

        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Edit));
        a.add_attr("b", 1, Some(Action::Edit));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_desc, b"desc a=val c=2 b=val");

        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Delete));
        a.add_attr("b", 1, Some(Action::Delete));
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, out| {
            out.extend_from_slice(b"val");
            Ok(())
        })
        .unwrap();
        assert_eq!(&out_desc, b"desc c=2");
    }

    #[test]
    fn del_multiple() {
        let mut out_id = vec![];
        let mut out_desc = vec![];
        let id = b"id";

        let mut a = Attrs::new(AttrFormat::new(b" ", b"="), SeqAttr::Desc);
        a.add_attr("a", 0, Some(Action::Delete));

        let desc = Some(&b"desc a=0"[..]);
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, _| Ok(()))
            .unwrap();
        assert_eq!(&out_desc, b"desc");

        let desc = Some(&b"desc2"[..]);
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, _| Ok(()))
            .unwrap();
        assert_eq!(&out_desc, b"desc2");

        let desc = Some(&b"a=4"[..]);
        a.parse(id, desc);
        a.compose(id, desc, &mut out_id, &mut out_desc, |_, _| Ok(()))
            .unwrap();
        assert_eq!(&out_desc, b"");
    }

    // #[bench]
    // fn bench_attr_parser(b: &mut test::Bencher) {
    //     let mut a = Attrs::new(b" ", b"=", SeqAttr::Desc);
    //     a.add_attr("a", 0, None);
    //     a.add_attr("b", 1, Some(Action::Edit));
    //     let id = b"id";
    //     let desc = Some(&b"asdf a=0 b=1"[..]);
    //     b.iter(|| {
    //         a.parse(id, desc);
    //     });
    // }
}
