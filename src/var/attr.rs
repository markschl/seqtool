use std::io;
use std::ops::Range;
use std::str::FromStr;

use memchr::memmem::{find, find_iter};
use winnow::combinator::{alt, rest, terminated};
use winnow::token::{take, take_until};
use winnow::{Located, PResult, Parser as _};

use crate::helpers::NA;
use crate::io::{MaybeModified, Record, RecordAttr};
use crate::{CliError, CliResult};

use super::symbols::SymbolTable;
use super::varstring::VarString;

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

/// Action to perform on header attributes when writing to output
#[derive(Debug, Clone, PartialEq)]
pub enum AttrWriteAction {
    /// Add the given VarString without overwriting (can lead to duplicates of the same attribute)
    Append(VarString),
    /// Replace with value of VarString if already present, otherwise append to the end
    Edit(VarString),
    /// Delete attribute
    Delete,
}

#[derive(Debug)]
pub struct Attributes {
    parser: Parser,
    // Since ID and description are parsed separately, we need to know if the delimiter before the attribute
    // is a space. In that case, attributes may also be found at the start of the description (which is, after a space)
    adelim_is_space: bool,
    // List of attribute IDs encountered in the record ID and description line.
    // These IDs allow accessing the attribute positions in the header with Parser::get(attr_id)
    id_attrs: Vec<usize>,
    desc_attrs: Vec<usize>,
    // // Intermediate buffer for descriptions, used in complicated cases where it is previously unknown, whether
    // // the description is empty or not, and therefore directly writing to the output is not possible.
    // desc_buf: Vec<u8>,
}

impl Attributes {
    pub fn new(format: AttrFormat) -> Attributes {
        let adelim_is_space = format.delim == b" ";
        Attributes {
            adelim_is_space,
            parser: Parser::new(format),
            id_attrs: Vec::new(),
            desc_attrs: Vec::new(),
            // desc_buf: Vec::new(),
        }
    }

    /// Requests an attribute to be searched in sequence headers.
    /// Returns the corresponding attribute ID (which is the index of the data in the array).
    /// The attribute may already exist, in which case the index (ID) of the existing slot is returned.
    pub fn add_attr(
        &mut self,
        name: &str,
        action: Option<AttrWriteAction>,
    ) -> Result<Option<usize>, String> {
        self.parser.register_attr(name, action)
    }

    // #[inline]
    // pub fn has_attrs(&self) -> bool {
    //     self.has_read_attrs() || self.has_append_attrs()
    // }

    #[inline]
    pub fn has_read_attrs(&self) -> bool {
        self.parser.n_read_edit_attrs() > 0
    }

    // #[inline]
    // pub fn has_append_attrs(&self) -> bool {
    //     self.parser.n_append_attrs() > 0
    // }

    pub fn parse(&mut self, record: &dyn Record) {
        // Only run the parser if necessary.
        // Also, calling `id_desc()` involves searching for the space delimiter,
        // (if not already done), we want to avoid that if possible.
        if self.has_read_attrs() {
            // In case of parsing attributes, we *always* search them in the ID/description parts
            // separately, instead of just searching the full header.
            // Reasons:
            // - (The ID/descriptions may already have been modified (we don't know the type of &dyn Record).
            //   However, with the current implementation, this is not the case.)
            // - The ID/descriptions may be modified later. Since they are modified separately,
            //   attributes can still be edited in the other part of the header. Appending is never
            //   a problem, even if modified.
            // - If the delimiter before the key=value attribute is a space, (the default!),
            //   e.g. '>id description key=value', then searching attributes in the
            //   full header line and *also* searching for the space to separate ID and description
            //   would mean that the same work is done twice.
            // - With non-space delimiters, the full header (ID and description) must be searched,
            //   and separating these parts is not necessarily required.
            //   But if also writing attributes to the output, they will be appended to the ID
            //   and not the description, so the end of the ID must again be known.
            //   Example: '>id;key=value some description'
            // Overall, it seems still easiest to do it separately, even if this may complicate the code here.
            let (id, desc) = record.id_desc();
            self._parse(id, desc);
            // dbg!(self);
        }
    }

    fn _parse(&mut self, id: &[u8], desc: Option<&[u8]>) {
        // initiate parser
        self.parser.init();
        // parse ID (everything before space) only if the delimiter before the key=value attribute
        // is not a space
        if !self.adelim_is_space {
            self.id_attrs.clear(); // TODO: where to clear?
            self.parser
                .parse(id, RecordAttr::Id, false, &mut self.id_attrs);
        }

        // the description (after space) is always searched
        if let Some(d) = desc {
            self.desc_attrs.clear();
            self.parser.parse(
                d,
                RecordAttr::Desc,
                self.adelim_is_space,
                &mut self.desc_attrs,
            );
        }
    }

    /// Writes the whole header line of the given record to output,
    /// adding in attributes where necesary.
    #[inline(always)]
    pub fn write_head<W: io::Write + ?Sized>(
        &self,
        record: &dyn Record,
        out: &mut W,
        symbols: &SymbolTable,
    ) -> CliResult<()> {
        let (id_head, opt_desc) = record.current_header().parts();

        if self.parser.n_write_attrs() == 0 {
            // nothing to do, just write the header
            // (either full header or separate ID/description parts, depending on what is available)
            out.write_all(&id_head)?;
            if let Some(d) = opt_desc.inner {
                out.write_all(b" ")?;
                out.write_all(d)?;
            }
            return Ok(());
        }
        // Otherwise, we need to modify something and write the modified header to `out`.
        self.write_head_with_attrs(id_head, opt_desc, record, out, symbols)
    }

    /// Writes the whole header line of the given record to output,
    /// adding in attributes where necesary.
    fn write_head_with_attrs<W: io::Write + ?Sized>(
        &self,
        id_head: MaybeModified<&[u8]>,
        opt_desc: MaybeModified<Option<&[u8]>>,
        record: &dyn Record,
        out: &mut W,
        symbols: &SymbolTable,
    ) -> CliResult<()> {
        // We can assume that `id_head` is the record ID, since the ID and description
        // parts were searched separately in `parse()`
        // ID
        #[inline(never)]
        fn mod_err(what: &str) -> CliError {
            format!(
                "Attempting to modify key=value attribute(s) in the record {0}, \
                but the {0} is simultaneously modified in another way",
                what
            )
            .into()
        }

        let any_written = if !self.id_attrs.is_empty() {
            if id_head.modified {
                return Err(mod_err("ID"));
            }
            self.parser
                .compose(&id_head, out, &self.id_attrs, symbols, record)?
        } else if !id_head.is_empty() {
            out.write_all(&id_head)?;
            true
        } else {
            false
        };
        if !self.adelim_is_space {
            // we append to the ID if the delimiter preceding the attribute is *not* a space
            self.parser
                .append_remaining(out, any_written, symbols, record)?;
        }

        // description
        if self.parser.has_append_attrs() && self.adelim_is_space || opt_desc.inner.is_some() {
            out.write_all(b" ")?;
        }
        let any_written = if let Some(desc) = opt_desc.inner {
            if !self.desc_attrs.is_empty() {
                if opt_desc.modified {
                    return Err(mod_err("description"));
                }
                self.parser
                    .compose(desc, out, &self.desc_attrs, symbols, record)?
            } else if !desc.is_empty() {
                out.write_all(desc)?;
                true
            } else {
                false
            }
        } else {
            debug_assert!(self.desc_attrs.is_empty());
            false
        };
        if self.adelim_is_space {
            // we append to the description if the delimiter preceding the attribute *is* a space
            self.parser
                .append_remaining(out, any_written, symbols, record)?;
        }
        Ok(())
    }

    pub fn has_value<'a>(
        &self,
        attr_id: usize,
        id_text: &'a [u8],
        desc_text: Option<&'a [u8]>,
    ) -> bool {
        if let Some(text) = self.get_value(attr_id, id_text, desc_text) {
            if text != NA.as_bytes() {
                return true;
            }
        }
        false
    }

    pub fn get_value<'a>(
        &self,
        attr_id: usize,
        id_text: &'a [u8],
        desc_text: Option<&'a [u8]>,
    ) -> Option<&'a [u8]> {
        let (_, position) = self.parser.get(attr_id);
        position.and_then(|(seq_attr, ref pos)| {
            let text = match seq_attr {
                RecordAttr::Id => id_text,
                RecordAttr::Desc => desc_text?,
                _ => panic!(),
            };
            Some(&text[pos.value_start..pos.end])
        })
    }
}

/// Parses and stores all attributes that have to be searched
/// (the names of these attributes is known beforehand).
/// After all requested attributes have been found, `parse` will stop
/// and ignore all additional attributes.
/// The attributes are stored in a vector, ordered by their attribute ID,
/// so obtaining values for attributes is a simple indexing operation if the
/// attribute ID is known.
#[derive(Debug)]
struct Parser {
    format: AttrFormat,
    // read and/or replace, delete
    read_edit_attrs: Vec<AttributeData>,
    n_edit_attrs: usize,
    has_delete_attrs: bool,
    n_write_attrs: usize,
    // append to end of ID or description: list of (name, value builder)
    append_attrs: Vec<(String, VarString)>,
    search_id: usize,
    num_found: usize,
    num_edit_attrs_found: usize,
    reported_duplicates: Vec<Vec<u8>>,
}

impl Parser {
    pub fn new(format: AttrFormat) -> Parser {
        Parser {
            format,
            read_edit_attrs: Vec::new(),
            n_edit_attrs: 0,
            has_delete_attrs: false,
            n_write_attrs: 0,
            append_attrs: Vec::new(),
            search_id: 0,
            num_found: 0,
            num_edit_attrs_found: 0,
            reported_duplicates: Vec::new(),
        }
    }

    /// Requests an attribute to be searched in sequence headers.
    /// Returns the corresponding attribute slot ("ID"), which is the index of the data
    /// in the array and is used to obtain the attribute value.
    /// Returns `None` if `write_action` is `Some(AttrWriteAction::Append)`,
    /// (means only writing, no parsing of the existing attribute(s)).
    /// The attribute may already exist, in which case the ID of the existing slot is returned.
    /// Multiple incompatible actions for the same attribute name will cause an error.
    /// - 'delete' conflicts with any other action to prevent inconsistent use
    /// - 'append' conflicts with any other action as well: if used in any other way (reading or writing),
    ///    appending is not a good idea. The attribute has to be searched, but will then be duplicated.
    pub fn register_attr(
        &mut self,
        name: &str,
        write_action: Option<AttrWriteAction>,
    ) -> Result<Option<usize>, String> {
        // dbg!(name, &write_action);
        // first check if any 'read/edit' actions are already present,
        // and if so return the corresponding slot ID
        #[inline(never)]
        fn dup_err<T>(name: &str) -> Result<T, String> {
            Err(format!(
                "The FASTA/FASTQ header attribute '{}' is added/edited twice",
                name
            ))
        }
        for (i, d) in self.read_edit_attrs.iter_mut().enumerate() {
            if d.name == name {
                if matches!(write_action, Some(AttrWriteAction::Edit(_)))
                    && matches!(d.write_action, Some(AttrWriteAction::Edit(_)))
                {
                    return dup_err(name);
                }
                if write_action != d.write_action {
                    if matches!(d.write_action, Some(AttrWriteAction::Delete))
                        || matches!(write_action, Some(AttrWriteAction::Delete))
                    {
                        return Err(format!(
                            "The FASTA/FASTQ header attribute '{}' is supposed to be deleted \
                            (e.g. using the `attr_del()` function), \
                            but the same attribute name is used in a different way. \
                            Make sure to use `attr_del` consistently at all places, \
                            and not to write this attribute the output using \
                            `-a/--attr {}=...` at the same time.",
                            name, name
                        ));
                    }
                    match write_action {
                        Some(AttrWriteAction::Append(_)) => return Err(format!(
                            "The FASTA/FASTQ header attribute '{}' is supposed to be appended \
                            to the header without first checking for its presence in the headers \
                            (`-A/--attr-append` argument; for maximum speed). \
                            However, the same attribute name is used in a different way as well. \
                            Make sure not to use functions such as `attr()`, `attr_del()` or `has_attr()` \
                            together with `-A/--attr-append`.", name,
                        )),
                        Some(AttrWriteAction::Edit(_)) => {
                            // replace 'read'-only (action = None) with the write action
                            d.write_action = write_action;
                            self.n_write_attrs += 1;
                            self.n_edit_attrs += 1;
                        }
                        _ => {}
                    }
                }
                return Ok(Some(i));
            }
        }
        // handle 'write actions'
        if write_action.is_some() {
            self.n_write_attrs += 1;
            match write_action {
                Some(AttrWriteAction::Delete) => {
                    self.has_delete_attrs = true;
                }
                Some(AttrWriteAction::Edit(_)) => {
                    self.n_edit_attrs += 1;
                }
                _ => {}
            }
        }
        if let Some(AttrWriteAction::Append(varstring)) = write_action {
            if self.append_attrs.iter().any(|(n, _)| n == name) {
                return dup_err(name);
            }
            self.append_attrs.push((name.to_string(), varstring));
            Ok(None)
        } else {
            let i = self.read_edit_attrs.len();
            self.read_edit_attrs.push(AttributeData {
                name: name.to_string(),
                // initial values, will be replaced
                rec_attr: RecordAttr::Id,
                position: AttrPosition::default(),
                write_action,
                search_id: usize::MAX, // should be different from self.search_id
            });
            Ok(Some(i))
        }
    }

    pub fn init(&mut self) {
        self.search_id = self.search_id.wrapping_add(1);
        self.num_found = 0;
        self.num_edit_attrs_found = 0;
    }

    /// Parses a given record header part (ID or description), searching for all
    /// key=value attributes of the given format.
    /// Also, fills the `hit_ids` vector with a list of attribute IDs
    /// (which are the slot indexes in the `data` vector)
    fn parse(
        &mut self,
        text: &[u8],
        rec_attr: RecordAttr,
        check_start: bool,
        hit_ids: &mut Vec<usize>,
    ) {
        if self.num_found == self.read_edit_attrs.len() {
            return;
        }
        let start_pos = if check_start { Some(0) } else { None };
        let pos_iter = start_pos
            .into_iter()
            .chain(find_iter(text, &self.format.delim).map(|pos| pos + 1));
        for pos in pos_iter {
            // at every delimiter position, we try to parse a key=value pair
            // or proceed with the next delimiter if not valid
            if let Some((key, _value, pos)) = parse_key_value(text, pos, &self.format) {
                // We do a linear search for the name, since we don't assume many attributes to be searched/added.
                for (i, d) in self.read_edit_attrs.iter_mut().enumerate() {
                    if key == d.name.as_bytes() {
                        if d.set_pos(rec_attr, pos, self.search_id) {
                            // attribute found and not a duplicate
                            self.num_found += 1;
                            hit_ids.push(i);
                            if matches!(d.write_action, Some(AttrWriteAction::Edit(_))) {
                                self.num_edit_attrs_found += 1;
                            }
                        } else {
                            // duplicate attribute found
                            // (this only happens if attribute parsing didn't stop earlier because
                            // all of them were found already)
                            if !self.reported_duplicates.iter().any(|name| name == key) {
                                eprintln!(
                                    "Warning: The FASTA/FASTQ header attribute '{}' was found to be \
                                    duplicated. Only the first occurrence is used. This can happen if \
                                    `-A/--attr-append` was used in an earlier command. \
                                    Note that not all duplicates are reported, there may be more...",
                                    String::from_utf8_lossy(key)
                                );
                                self.reported_duplicates.push(key.to_owned());
                            }
                        }
                        break;
                    }
                }
                if self.num_found == self.read_edit_attrs.len() {
                    break;
                }
            }
        }
    }

    // Compose a header part (ID or description) by editing and/or deleting attributes
    fn compose<W: io::Write + ?Sized>(
        &self,
        text: &[u8],
        out: &mut W,
        hit_ids: &[usize],
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> io::Result<bool> {
        let mut prev_end = 0;
        let mut any_written = false;
        macro_rules! do_write {
            ($text: expr) => {
                if !$text.is_empty() {
                    if !any_written {
                        any_written = true;
                    }
                    out.write_all($text)?;
                }
            };
        }
        // if self.num_found > 0 {
        for attr_id in hit_ids {
            let d = &self.read_edit_attrs[*attr_id];
            debug_assert!(d.get_pos(self.search_id).is_some());
            match &d.write_action {
                Some(AttrWriteAction::Edit(vs)) => {
                    out.write_all(&text[prev_end..d.position.value_start])?;
                    vs.compose(out, symbols, record)?;
                    any_written = true;
                }
                Some(AttrWriteAction::Delete) => {
                    // remove preceding delimiter, unless start = 0
                    let end = if d.position.start > prev_end {
                        d.position.start - 1
                    } else {
                        d.position.start
                    };
                    do_write!(&text[prev_end..end]);
                }
                _ => continue,
            }
            prev_end = d.position.end;
        }
        // }
        do_write!(&text[prev_end..]);
        Ok(any_written)
    }

    fn has_append_attrs(&self) -> bool {
        !self.append_attrs.is_empty() || self.num_edit_attrs_found < self.n_edit_attrs
    }

    // fn has_delete_attrs(&self) -> bool {
    //     self.has_delete_attrs
    // }

    /// Appends attributes of type `Edit` that were not found in the record, or `Append` attributes
    /// to the output (which may be the ID or description)
    /// `delim_before`: should the first attribute have a delimiter before it?
    fn append_remaining<W: io::Write + ?Sized>(
        &self,
        out: &mut W,
        initial_delim: bool,
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> io::Result<()> {
        // the 'edit' attributes *not found* in the header come first
        // TODO: if num_found == self.read_edit_attrs.len() this search is actually unnecessary
        let attr_iter = self
            .read_edit_attrs
            .iter()
            .filter_map(|d| {
                if d.get_pos(self.search_id).is_none() {
                    if let Some(AttrWriteAction::Edit(vs)) = &d.write_action {
                        return Some((d.name.as_str(), vs));
                    }
                }
                None
            })
            // next, the 'append' attributes
            .chain(
                self.append_attrs
                    .iter()
                    .map(|(name, vs)| (name.as_str(), vs)),
            )
            .enumerate();
        // do the writing
        for (i, (name, vs)) in attr_iter {
            if initial_delim || i != 0 {
                // TODO: check position?
                out.write_all(&self.format.delim)?;
            }
            out.write_all(name.as_bytes())?;
            out.write_all(&self.format.value_delim)?;
            vs.compose(out, symbols, record)?;
        }
        Ok(())
    }

    pub fn get(&self, attr_id: usize) -> (&str, Option<(RecordAttr, AttrPosition)>) {
        let d = self.read_edit_attrs.get(attr_id).unwrap();
        (&d.name, d.get_pos(self.search_id))
    }

    // pub fn num_found(&self) -> usize {
    //     self.num_found
    // }

    pub fn n_write_attrs(&self) -> usize {
        self.n_write_attrs
    }

    pub fn n_read_edit_attrs(&self) -> usize {
        self.read_edit_attrs.len()
    }

    // pub fn n_append_attrs(&self) -> usize {
    //     self.append_attrs.len()
    // }
}

/// Object holding information about attributes that we want to find,
/// as well as information about the position of the hit (if found)
#[derive(Debug)]
struct AttributeData {
    // attribute name
    name: String,
    // The record attribute (ID/desc), in which the hit was found
    rec_attr: RecordAttr,
    // Positional information, changes with each record.
    position: AttrPosition,
    /// The action to perform when writing to output
    write_action: Option<AttrWriteAction>,
    // Search ID: used to know if the position is up-to date
    // (instead of resetting before each record)
    // The search ID is essentially the record index, which will
    // restart at 0 when the index overflows (using wrapping_add(), see parser)
    search_id: usize,
}

impl AttributeData {
    fn get_pos(&self, search_id: usize) -> Option<(RecordAttr, AttrPosition)> {
        if search_id == self.search_id {
            Some((self.rec_attr, self.position.clone()))
        } else {
            None
        }
    }

    /// Sets the position for the given search ID.
    /// Returns true if the position did not previously exist in this search
    /// (so it is not a duplicate hit)
    fn set_pos(&mut self, attr: RecordAttr, pos: AttrPosition, search_id: usize) -> bool {
        if search_id != self.search_id {
            // position was not yet found in this round
            self.rec_attr = attr;
            self.position = pos;
            self.search_id = search_id;
            return true;
        }
        false
    }
}

/// Position of an attribute in the header ID or description
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

#[cfg(test)]
mod tests {
    // use seq_io::fasta::OwnedRecord;

    use crate::{
        io::{MaybeModified, OwnedRecord},
        var::varstring::VarStringSegment,
    };

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

    fn compose_head(
        id: &[u8],
        desc: Option<&[u8]>,
        fmt: AttrFormat,
        mut add_fn: impl FnMut(&mut Attributes),
    ) -> CliResult<Vec<u8>> {
        let mut a = Attributes::new(fmt);
        add_fn(&mut a);
        let rec = OwnedRecord {
            id: MaybeModified::new(id.to_vec(), false),
            desc: MaybeModified::new(desc.map(|d| d.to_vec()), false),
            seq: Vec::new(),
            qual: None,
        };
        a.parse(&rec);
        let sym = SymbolTable::new(0);
        let mut out = Vec::new();
        a.write_head(&rec, &mut out, &sym)?;
        Ok(out)
    }

    #[test]
    fn desc_parser() {
        let fmt = AttrFormat::new(b" ", b"=");
        let repl = VarString::from_segments(&[VarStringSegment::Text(b"val".to_vec())]);

        let head = compose_head(b"id", Some(&b"desc a=0 b=1"[..]), fmt.clone(), |a| {
            a.add_attr("a", None).unwrap();
            a.add_attr("b", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc a=0 b=val");

        let head = compose_head(b"id", Some(&b"desc a=0 c=1"[..]), fmt.clone(), |a| {
            a.add_attr("a", None).unwrap();
            a.add_attr("b", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc a=0 c=1 b=val");

        let head = compose_head(b"id", Some(&b"desc a=0 b=1"[..]), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Delete)).unwrap();
            a.add_attr("b", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc b=val");
    }

    #[test]
    fn delim() {
        let fmt = AttrFormat::new(b";", b"=");
        let repl = VarString::from_segments(&[VarStringSegment::Text(b"val".to_vec())]);

        let head = compose_head(b"id;a=0", Some(&b"desc a:1"[..]), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id;a=val desc a:1");

        let fmt = AttrFormat::new(b" ", b":");
        let head = compose_head(b"id;a=0", Some(&b"desc a:1"[..]), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id;a=0 desc a:val");
    }

    #[test]
    fn missing() {
        let fmt = AttrFormat::new(b" ", b"=");
        let repl = VarString::from_segments(&[VarStringSegment::Text(b"val".to_vec())]);

        let head = compose_head(b"id", Some(&b"desc a=0 c=2"[..]), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
            a.add_attr("b", Some(AttrWriteAction::Edit(repl.clone())))
                .unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc a=val c=2 b=val");

        let head = compose_head(b"id", Some(&b"desc a=0 c=2"[..]), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Delete)).unwrap();
            a.add_attr("b", Some(AttrWriteAction::Delete)).unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc c=2");
    }

    #[test]
    fn del_multiple() {
        let fmt = AttrFormat::new(b" ", b"=");

        let head = compose_head(b"id", Some(b"desc a=0"), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Delete)).unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc");

        let head = compose_head(b"id", Some(b"desc2"), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Delete)).unwrap();
        })
        .unwrap();
        assert_eq!(&head, b"id desc2");

        let head = compose_head(b"id", Some(b"a=4"), fmt.clone(), |a| {
            a.add_attr("a", Some(AttrWriteAction::Delete)).unwrap();
        })
        .unwrap();
        // TODO: the extra space should be removed
        assert_eq!(&head, b"id ");
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
