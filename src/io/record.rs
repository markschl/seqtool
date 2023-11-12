use std::{
    borrow::Cow,
    str::{self, Utf8Error},
};

use seq_io::fasta;
use strum_macros::{Display, EnumString};

pub trait Record {
    fn id_bytes(&self) -> &[u8];
    fn desc_bytes(&self) -> Option<&[u8]>;
    // TODO: id_desc_bytes()
    fn raw_seq(&self) -> &[u8];
    fn qual(&self) -> Option<&[u8]>;
    fn has_seq_lines(&self) -> bool;
    fn get_header(&self) -> SeqHeader;
    /// Returns the position of the space delimiter in the FASTA/FASTQ header,
    /// if already searched (outer option) and present (inner option)
    /// This option is used by the FastaRecord and FastqRecord wrappers
    /// in order to cache the position of the ID/description
    fn delim(&self) -> Option<Option<usize>> {
        None
    }
    /// Sets the position of the space delimiter in the FASTA/FASTQ header
    /// if it is already known in order to prevent it from being searched again.
    /// This is used for parallel processing, in case of accessing the ID
    /// within the worker thread, its position will not have to be searched
    /// again in the main thread.
    fn set_delim(&self, _: Option<usize>) {}
    /// Iterator over sequence lines (for FASTA), or just the sequence (FASTQ/CSV).
    /// The idea is to prevent allocations and copying, otherwise use `write_seq`
    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Other(Some(self.raw_seq()))
    }

    fn full_seq<'a>(&'a self, buf: &'a mut Vec<u8>) -> Cow<'a, [u8]> {
        if !self.has_seq_lines() {
            self.raw_seq().into()
        } else {
            buf.clear();
            for seq in self.seq_segments() {
                buf.extend_from_slice(seq);
            }
            buf.as_slice().into()
        }
    }

    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (self.id_bytes(), self.desc_bytes())
    }

    fn id(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(self.id_bytes())
    }

    fn desc(&self) -> Option<Result<&str, Utf8Error>> {
        self.desc_bytes().map(str::from_utf8)
    }

    fn desc_or<'a>(&'a self, default: &'a str) -> Result<&'a str, Utf8Error> {
        self.desc_bytes()
            .map(str::from_utf8)
            .unwrap_or_else(|| Ok(default))
    }

    fn seq_len(&self) -> usize {
        self.seq_segments().fold(0, |l, s| l + s.len())
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        for seq in self.seq_segments() {
            to.extend_from_slice(seq);
        }
    }

    /// Writes the contents of the given sequence attribute to out
    fn write_attr(&self, attr: SeqAttr, out: &mut Vec<u8>) {
        match attr {
            SeqAttr::Id => {
                out.extend_from_slice(self.id_bytes());
            }
            SeqAttr::Desc => {
                if let Some(d) = self.desc_bytes() {
                    out.extend_from_slice(d);
                }
            }
            SeqAttr::Seq => {
                for s in self.seq_segments() {
                    out.extend_from_slice(s);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum SeqHeader<'a> {
    IdDesc(&'a [u8], Option<&'a [u8]>),
    FullHeader(&'a [u8]),
}

/// Not to be confused with key=value attributes
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, EnumString, Display)]
pub enum SeqAttr {
    Id,
    Desc,
    Seq,
}

// impl SeqAttr {
//     pub fn from_str(attr: &str) -> Option<SeqAttr> {
//         Some(if attr.eq_ignore_ascii_case("id") {
//             SeqAttr::Id
//         } else if attr.eq_ignore_ascii_case("desc") {
//             SeqAttr::Desc
//         } else if attr.eq_ignore_ascii_case("seq") {
//             SeqAttr::Seq
//         } else {
//             return None;
//         })
//     }
// }

impl<'b, R: Record + ?Sized> Record for &'b R {
    fn id_bytes(&self) -> &[u8] {
        (**self).id_bytes()
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        (**self).desc_bytes()
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (**self).id_desc_bytes()
    }
    fn delim(&self) -> Option<Option<usize>> {
        (**self).delim()
    }
    fn set_delim(&self, delim: Option<usize>) {
        (**self).set_delim(delim)
    }
    fn raw_seq(&self) -> &[u8] {
        (**self).raw_seq()
    }
    fn qual(&self) -> Option<&[u8]> {
        (**self).qual()
    }
    fn has_seq_lines(&self) -> bool {
        (**self).has_seq_lines()
    }
    fn seq_segments(&self) -> SeqLineIter {
        (**self).seq_segments()
    }
    fn full_seq<'a>(&'a self, buf: &'a mut Vec<u8>) -> Cow<'a, [u8]> {
        (**self).full_seq(buf)
    }
    fn get_header(&self) -> SeqHeader {
        (**self).get_header()
    }
}

pub enum SeqLineIter<'a> {
    Fasta(fasta::SeqLines<'a>),
    Other(Option<&'a [u8]>),
}

impl<'a> Iterator for SeqLineIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        match *self {
            SeqLineIter::Fasta(ref mut l) => l.next(),
            SeqLineIter::Other(ref mut o) => o.take(),
        }
    }
}

impl<'a> DoubleEndedIterator for SeqLineIter<'a> {
    fn next_back(&mut self) -> Option<&'a [u8]> {
        match *self {
            SeqLineIter::Fasta(ref mut l) => l.next_back(),
            SeqLineIter::Other(ref mut o) => o.take(),
        }
    }
}

// Wrapper storing custom IDs / descriptions

pub struct DefRecord<'a, R: Record> {
    rec: R,
    id: &'a [u8],
    desc: Option<&'a [u8]>,
}

impl<'a, R: Record + 'a> DefRecord<'a, R> {
    pub fn new(inner: R, id: &'a [u8], desc: Option<&'a [u8]>) -> DefRecord<'a, R> {
        DefRecord {
            rec: inner,
            id,
            desc,
        }
    }
}

impl<'b, R: Record> Record for DefRecord<'b, R> {
    fn id_bytes(&self) -> &[u8] {
        self.id
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.desc
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (self.id, self.desc)
    }
    fn delim(&self) -> Option<Option<usize>> {
        self.rec.delim()
    }
    fn set_delim(&self, delim: Option<usize>) {
        self.rec.set_delim(delim)
    }
    fn get_header(&self) -> SeqHeader {
        SeqHeader::IdDesc(self.id, self.desc)
    }
    fn raw_seq(&self) -> &[u8] {
        self.rec.raw_seq()
    }
    fn has_seq_lines(&self) -> bool {
        self.rec.has_seq_lines()
    }
    fn qual(&self) -> Option<&[u8]> {
        self.rec.qual()
    }
    fn seq_segments(&self) -> SeqLineIter {
        self.rec.seq_segments()
    }
}

// Wrapper storing sequence/quality data

pub struct SeqQualRecord<'a, R: Record> {
    rec: R,
    seq: &'a [u8],
    qual: Option<&'a [u8]>,
}

impl<'a, R: Record + 'a> SeqQualRecord<'a, R> {
    pub fn new(inner: R, seq: &'a [u8], qual: Option<&'a [u8]>) -> SeqQualRecord<'a, R> {
        SeqQualRecord {
            rec: inner,
            seq,
            qual,
        }
    }
}

impl<'b, R: Record> Record for SeqQualRecord<'b, R> {
    fn id_bytes(&self) -> &[u8] {
        self.rec.id_bytes()
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.rec.desc_bytes()
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        self.rec.id_desc_bytes()
    }
    fn get_header(&self) -> SeqHeader {
        self.rec.get_header()
    }
    fn delim(&self) -> Option<Option<usize>> {
        self.rec.delim()
    }
    fn set_delim(&self, delim: Option<usize>) {
        self.rec.set_delim(delim)
    }
    fn raw_seq(&self) -> &[u8] {
        self.seq
    }
    fn has_seq_lines(&self) -> bool {
        self.rec.has_seq_lines()
    }
    fn qual(&self) -> Option<&[u8]> {
        self.qual.or_else(|| self.rec.qual())
    }
    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Other(Some(self.seq))
    }
}

// Record that owns all data

#[derive(Default, Clone)]
pub struct OwnedRecord {
    pub id: Vec<u8>,
    pub desc: Option<Vec<u8>>,
    pub seq: Vec<u8>,
    pub qual: Option<Vec<u8>>,
}

impl Record for OwnedRecord {
    fn id_bytes(&self) -> &[u8] {
        &self.id
    }
    fn desc_bytes(&self) -> Option<&[u8]> {
        self.desc.as_deref()
    }
    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (self.id_bytes(), self.desc_bytes())
    }
    fn get_header(&self) -> SeqHeader {
        SeqHeader::IdDesc(self.id_bytes(), self.desc_bytes())
    }
    fn raw_seq(&self) -> &[u8] {
        &self.seq
    }
    fn qual(&self) -> Option<&[u8]> {
        self.qual.as_deref()
    }
    fn has_seq_lines(&self) -> bool {
        false
    }
}

// Wrapper for retrieving and editing record attributes

#[derive(Debug, Default)]
pub struct RecordEditor {
    id: Option<Vec<u8>>,
    desc: Option<Vec<u8>>,
    seq: Option<Vec<u8>>,
    seq_cache: SeqCache,
}

#[derive(Debug, Default)]
struct SeqCache(Vec<u8>);

impl SeqCache {
    fn get_seq<'a>(&'a mut self, rec: &'a dyn Record, get_cached: bool) -> &'a [u8] {
        if rec.has_seq_lines() {
            if get_cached {
                self.0.clear();
            }
            rec.write_seq(&mut self.0);
            &self.0
        } else {
            rec.raw_seq()
        }
    }
}

impl RecordEditor {
    pub fn new() -> RecordEditor {
        RecordEditor {
            id: None,
            desc: None,
            seq: None,
            seq_cache: SeqCache(vec![]),
        }
    }

    #[inline]
    pub fn get<'a>(&'a mut self, attr: SeqAttr, rec: &'a dyn Record, get_cached: bool) -> &'a [u8] {
        match attr {
            SeqAttr::Id => rec.id_bytes(),
            SeqAttr::Desc => rec.desc_bytes().unwrap_or(b""),
            SeqAttr::Seq => self.seq_cache.get_seq(rec, get_cached),
        }
    }

    #[inline]
    pub fn edit(&mut self, attr: SeqAttr) -> &mut Vec<u8> {
        let v = match attr {
            SeqAttr::Id => self.id.get_or_insert_with(Vec::new),
            SeqAttr::Desc => self.desc.get_or_insert_with(Vec::new),
            SeqAttr::Seq => self.seq.get_or_insert_with(Vec::new),
        };
        v.clear();
        v
    }

    #[inline]
    pub fn edit_with_val<F, O>(
        &mut self,
        attr: SeqAttr,
        rec: &dyn Record,
        get_cached: bool,
        mut func: F,
    ) -> O
    where
        F: FnMut(&[u8], &mut Vec<u8>) -> O,
    {
        match attr {
            SeqAttr::Id => {
                let v = self.id.get_or_insert_with(Vec::new);
                v.clear();
                func(rec.id_bytes(), v)
            }
            SeqAttr::Desc => {
                let v = self.desc.get_or_insert_with(Vec::new);
                v.clear();
                func(rec.desc_bytes().unwrap_or(b""), v)
            }
            SeqAttr::Seq => {
                let seq = self.seq_cache.get_seq(rec, get_cached);
                let v = self.seq.get_or_insert_with(Vec::new);
                v.clear();
                func(seq, v)
            }
        }
    }

    #[inline]
    pub fn rec<'r>(&'r self, rec: &'r dyn Record) -> EditedRecord<'r> {
        EditedRecord { editor: self, rec }
    }
}

pub struct EditedRecord<'a> {
    editor: &'a RecordEditor,
    rec: &'a dyn Record,
}

impl<'r> Record for EditedRecord<'r> {
    fn id_bytes(&self) -> &[u8] {
        self.editor
            .id
            .as_deref()
            .unwrap_or_else(|| self.rec.id_bytes())
    }

    fn desc_bytes(&self) -> Option<&[u8]> {
        self.editor
            .desc
            .as_ref()
            .map(|d| Some(d.as_slice()))
            .unwrap_or_else(|| self.rec.desc_bytes())
    }

    fn id_desc_bytes(&self) -> (&[u8], Option<&[u8]>) {
        (self.id_bytes(), self.rec.desc_bytes())
    }
    fn get_header(&self) -> SeqHeader {
        if self.editor.id.is_some() || self.editor.desc.is_some() {
            SeqHeader::IdDesc(self.id_bytes(), self.desc_bytes())
        } else {
            self.rec.get_header()
        }
    }

    fn delim(&self) -> Option<Option<usize>> {
        self.rec.delim()
    }
    fn set_delim(&self, delim: Option<usize>) {
        self.rec.set_delim(delim)
    }

    fn raw_seq(&self) -> &[u8] {
        self.editor
            .seq
            .as_deref()
            .unwrap_or_else(|| self.rec.raw_seq())
    }

    fn has_seq_lines(&self) -> bool {
        if self.editor.seq.is_some() {
            false
        } else {
            self.rec.has_seq_lines()
        }
    }

    fn qual(&self) -> Option<&[u8]> {
        self.rec.qual()
    }

    fn seq_segments(&self) -> SeqLineIter {
        self.editor
            .seq
            .as_ref()
            .map(|s| SeqLineIter::Other(Some(s)))
            .unwrap_or_else(|| self.rec.seq_segments())
    }
}
