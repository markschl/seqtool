use io::Attribute;

use memchr::Memchr;

use lib::key_value;

#[derive(Debug, Clone)]
pub struct PropPosition {
    pub start: usize,
    pub value_start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct Parser {
    delim: u8,
    value_delim: u8,
}

impl Parser {
    pub fn new(delim: u8, value_delim: u8) -> Parser {
        Parser {
            delim: delim,
            value_delim: value_delim,
        }
    }

    fn next_pos<'a>(&self, text: &'a [u8], pos: usize) -> Option<(&'a [u8], PropPosition)> {

        for p in Memchr::new(self.delim, &text[pos..]) {
            if let Some(out) = self.check_pos(text, pos + p + 1) {
                return Some(out);
            }
        }
        None
    }

    fn check_pos<'a>(&self, text: &'a [u8], pos: usize) -> Option<(&'a [u8], PropPosition)> {
        key_value::parse(&text[pos..], self.delim, self.value_delim)
            .map(|(key, vstart, end)| {
                (
                key,
                PropPosition {
                    start: pos,
                    value_start: pos + vstart,
                    end: pos + end,
                },
            )})
    }

    #[inline]
    pub fn parse<'a>(&'a self, text: &'a [u8], search_start: bool) -> PropIter<'a> {
        PropIter {
            parser: self,
            text: text,
            pos: 0,
            search_start: search_start,
        }
    }
}

pub struct PropIter<'a> {
    parser: &'a Parser,
    text: &'a [u8],
    pos: usize,
    search_start: bool,
}

impl<'a> Iterator for PropIter<'a> {
    type Item = (&'a [u8], PropPosition);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == 0 && self.search_start {
            self.search_start = false;
            if let Some(p) = self.parser.check_pos(self.text, 0) {
                return Some(p);
            }
        }
        self.parser.next_pos(self.text, self.pos).map(|p| {
            self.pos = p.1.end;
            p
        })
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Action {
    Edit,
    Delete,
}

#[derive(Debug)]
pub struct Props {
    parser: Parser,
    // TODO: complicated data structure
    props: Vec<
        (
            (String, Option<Action>),
            ((Attribute, (usize, usize)), usize),
        ),
    >,
    id_props: Vec<(PropPosition, usize, Action)>,
    desc_props: Vec<(PropPosition, usize, Action)>,
    search_id: usize,
    prop_delim: u8,
    prop_value_delim: u8,
    // the distinction of ID and description makes handling of spaces somehow complicated
    pdelim_is_space: bool,
    prop_append_attr: Attribute,
}

impl Props {
    pub fn new(prop_delim: u8, prop_value_delim: u8, prop_append_attr: Attribute) -> Props {
        Props {
            props: vec![],
            parser: Parser::new(prop_delim, prop_value_delim),
            id_props: vec![],
            desc_props: vec![],
            search_id: 1,
            prop_delim: prop_delim,
            prop_value_delim: prop_value_delim,
            pdelim_is_space: prop_delim == b' ',
            prop_append_attr: prop_append_attr,
        }
    }

    // Not a "smart" function, names must be added in order of IDs (just supplied to ensure
    // consistency). Only used for importing properties from VarStore, which assigns the IDs
    pub fn add_prop(&mut self, name: &str, id: usize, action: Option<Action>) {
        assert!(id == self.props.len());
        self.props.insert(
            id,
            ((name.to_string(), action), ((Attribute::Id, (0, 0)), 0)),
        );
    }

    #[inline]
    pub fn has_props(&self) -> bool {
        !self.props.is_empty()
    }

    #[inline]
    pub fn parse(&mut self, id: &[u8], desc: Option<&[u8]>) {
        self.search_id += 1;
        let n = self.props.len();
        let n_found =
            if self.prop_delim == b' ' {
                // no need to search for space in ID
                0
            } else {
                parse(id, self.props.as_mut_slice(), &mut self.id_props,
                    Attribute::Id, n, self.search_id, &mut self.parser, false)
            };

        if let Some(d) = desc {
            parse(d, self.props.as_mut_slice(), &mut self.desc_props,
                Attribute::Desc, n - n_found, self.search_id, &mut self.parser, self.pdelim_is_space);
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

        self._compose(id, &self.id_props, out_id, &mut push_fn);

        if let Some(d) = desc {
            self._compose(d, &self.desc_props, out_desc, &mut push_fn);
        }

        if self.prop_append_attr == Attribute::Id {
            self.append_missing(out_id, true, &mut push_fn);

        } else if self.prop_append_attr == Attribute::Desc {
            let delim_before = !(self.pdelim_is_space && out_desc.is_empty());
            self.append_missing(out_desc, delim_before, &mut push_fn);
        }
    }

    fn _compose<F>(
        &self,
        text: &[u8],
        positions: &[(PropPosition, usize, Action)],
        new_text: &mut Vec<u8>,
        mut push_fn: F,
    ) where
        F: FnMut(usize, &mut Vec<u8>),
    {
        let mut prev_end = 0;

        for &(ref pos, id, action) in positions {
            match action {
                Action::Edit => {
                    new_text.extend_from_slice(&text[prev_end..pos.value_start]);
                    push_fn(id, new_text);
                    prev_end = pos.end;
                }
                Action::Delete => {
                    // remove the delimiter before if possible, but pos.start == 0 is also possible
                    let end = if pos.start > prev_end {
                        pos.start - 1
                    } else {
                        pos.start
                    };
                    new_text.extend_from_slice(&text[prev_end..end]);
                    prev_end = pos.end;
                }
            }
        }
        new_text.extend_from_slice(&text[prev_end..]);
    }

    fn append_missing<F>(&self, new_text: &mut Vec<u8>, delim_before: bool, mut push_fn: F)
    where
        F: FnMut(usize, &mut Vec<u8>),
    {
        let mut delim_before = delim_before;
        for (i, &((ref name, _), (_, _search_id))) in self.props.iter().enumerate() {
            if _search_id != self.search_id {
                if delim_before {
                    new_text.push(self.prop_delim);
                } else {
                    delim_before = true;
                }
                new_text.extend_from_slice(name.as_bytes());
                new_text.push(self.prop_value_delim);
                push_fn(i, new_text);
            }
        }
    }

    pub fn get<'a>(&self, id: usize, id_text: &'a [u8], desc_text: Option<&'a [u8]>)
        -> Option<&'a [u8]>
    {
        self.props
            .get(id)
            .and_then(|&(_, ((attr, (start, end)), _search_id))| {
                if _search_id == self.search_id {
                    let text = match attr {
                        Attribute::Id => id_text,
                        Attribute::Desc => {
                            if let Some(d) = desc_text {
                                d
                            } else {
                                return None;
                            }
                        }
                        _ => panic!(),
                    };
                    Some(&text[start..end])
                } else {
                    None
                }
            })
    }
}

// responsible for parsing properties within a text and filling them into 'props'
// cannot have this as `Props` method because of borrow issues
// TODO: ugly and complicated code
fn parse(
    text: &[u8],
    props: &mut [(
        (String, Option<Action>),
        ((Attribute, (usize, usize)), usize),
    )],
    full_pos: &mut Vec<(PropPosition, usize, Action)>,
    attr: Attribute,
    max_find: usize,
    search_id: usize,
    parser: &mut Parser,
    search_start: bool,
) -> usize {
    full_pos.clear();
    for (name, pos) in parser.parse(text, search_start) {
        for (i, &mut ((ref prop_name, action), (ref mut out, ref mut _search_id))) in
            props.iter_mut().enumerate()
        {
            if name == prop_name.as_bytes() {
                if *_search_id != search_id {
                    *out = (attr, (pos.value_start, pos.end));
                    *_search_id = search_id;
                    if let Some(a) = action {
                        full_pos.push((pos, i, a));
                    }
                }

                if full_pos.len() == max_find {
                    return full_pos.len();
                }
                break;
            }
        }
    }
    full_pos.len()
}
