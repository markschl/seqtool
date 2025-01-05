use std::ops::DerefMut;
use std::str;
use std::{borrow::Cow, ops::Deref};

use seq_io::fasta;
use strum_macros::{Display, EnumString};

pub trait Record {
    fn id(&self) -> &[u8];
    fn desc(&self) -> Option<&[u8]>;
    /// Returns ID/description parts of the header, searching for the space
    /// separator if not already done due to previous call to `desc()`
    fn id_desc(&self) -> (&[u8], Option<&[u8]>);
    /// Returns ID/description parts (which are separated by space)
    /// if the space was already searched in the FASTA/FASTQ headers.
    /// Otherwise, returns (full header, None).
    /// For delimited text, (ID, desc) is returned (desc is optional);
    /// no full header is availble there, although the ID may also contain spaces.
    fn current_header(&self) -> RecordHeader;
    /// Raw sequence that may contain line breaks
    fn raw_seq(&self) -> &[u8];
    /// Quality line (without line breaks)
    fn qual(&self) -> Option<&[u8]>;
    /// Returns the position of the space delimiter in the FASTA/FASTQ header,
    /// if already searched (outer option) and present (inner option)
    /// This is used by the FastaRecord and FastqRecord wrappers
    /// in order to cache the position of the ID/description.
    fn header_delim_pos(&self) -> Option<Option<usize>> {
        None
    }
    /// Sets the position of the space delimiter in the FASTA/FASTQ header
    /// if it is already known in order to prevent it from being searched again.
    /// This is used for parallel processing, in case of accessing the ID
    /// within the worker thread, its position will not have to be searched
    /// again in the main thread.
    fn set_header_delim_pos(&self, _: Option<usize>) {}
    /// Does the sequence have >1 lines?
    /// Default impl: only one line, see also seq_segments()
    fn has_seq_lines(&self) -> bool {
        false
    }
    /// Iterator over sequence lines (for FASTA), or just the sequence (FASTQ/CSV).
    /// The idea is to prevent allocations and copying, otherwise use `write_seq`
    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::OneLine(Some(self.raw_seq()))
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

    fn seq_len(&self) -> usize {
        self.seq_segments().fold(0, |l, s| l + s.len())
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        for seq in self.seq_segments() {
            to.extend_from_slice(seq);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MaybeModified<T> {
    pub inner: T,
    pub modified: bool,
}

impl<T> MaybeModified<T> {
    pub fn new(inner: T, modified: bool) -> Self {
        Self { inner, modified }
    }
}

impl<T> Deref for MaybeModified<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for MaybeModified<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Clone, Debug)]
pub enum RecordHeader<'a> {
    IdDesc(MaybeModified<&'a [u8]>, MaybeModified<Option<&'a [u8]>>),
    Full(&'a [u8]),
}

impl<'a> RecordHeader<'a> {
    pub fn parts(&self) -> (MaybeModified<&'a [u8]>, MaybeModified<Option<&'a [u8]>>) {
        match self {
            RecordHeader::IdDesc(id, desc) => (id.clone(), desc.clone()),
            RecordHeader::Full(h) => (
                MaybeModified::new(h, false),
                MaybeModified::new(None, false),
            ),
        }
    }
}

/// Not to be confused with key=value attributes
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, EnumString, Display)]
pub enum RecordAttr {
    Id,
    Desc,
    Seq,
}

impl<R: Record + ?Sized> Record for &R {
    fn id(&self) -> &[u8] {
        (**self).id()
    }

    fn desc(&self) -> Option<&[u8]> {
        (**self).desc()
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        (**self).id_desc()
    }

    fn current_header(&self) -> RecordHeader {
        (**self).current_header()
    }

    fn raw_seq(&self) -> &[u8] {
        (**self).raw_seq()
    }

    fn qual(&self) -> Option<&[u8]> {
        (**self).qual()
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        (**self).header_delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        (**self).set_header_delim_pos(delim)
    }

    fn has_seq_lines(&self) -> bool {
        (**self).has_seq_lines()
    }

    fn seq_segments(&self) -> SeqLineIter {
        (**self).seq_segments()
    }
}

pub enum SeqLineIter<'a> {
    Fasta(fasta::SeqLines<'a>),
    OneLine(Option<&'a [u8]>),
}

impl<'a> Iterator for SeqLineIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        match *self {
            SeqLineIter::Fasta(ref mut l) => l.next(),
            SeqLineIter::OneLine(ref mut o) => o.take(),
        }
    }
}

impl<'a> DoubleEndedIterator for SeqLineIter<'a> {
    fn next_back(&mut self) -> Option<&'a [u8]> {
        match *self {
            SeqLineIter::Fasta(ref mut l) => l.next_back(),
            SeqLineIter::OneLine(ref mut o) => o.take(),
        }
    }
}

// Wrapper storing custom IDs / descriptions

pub struct HeaderRecord<'a, R: Record> {
    rec: R,
    id: &'a [u8],
    desc: Option<&'a [u8]>,
}

impl<'a, R: Record + 'a> HeaderRecord<'a, R> {
    pub fn new(inner: R, id: &'a [u8], desc: Option<&'a [u8]>) -> HeaderRecord<'a, R> {
        HeaderRecord {
            rec: inner,
            id,
            desc,
        }
    }
}

impl<R: Record> Record for HeaderRecord<'_, R> {
    fn id(&self) -> &[u8] {
        self.id
    }

    fn desc(&self) -> Option<&[u8]> {
        self.desc
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        (self.id, self.desc)
    }

    fn current_header(&self) -> RecordHeader {
        RecordHeader::IdDesc(
            MaybeModified::new(self.id, true),
            MaybeModified::new(self.desc, true),
        )
    }

    fn raw_seq(&self) -> &[u8] {
        self.rec.raw_seq()
    }

    fn qual(&self) -> Option<&[u8]> {
        self.rec.qual()
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.rec.header_delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.rec.set_header_delim_pos(delim)
    }

    fn has_seq_lines(&self) -> bool {
        self.rec.has_seq_lines()
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

impl<R: Record> Record for SeqQualRecord<'_, R> {
    fn id(&self) -> &[u8] {
        self.rec.id()
    }

    fn desc(&self) -> Option<&[u8]> {
        self.rec.desc()
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        self.rec.id_desc()
    }

    fn current_header(&self) -> RecordHeader {
        self.rec.current_header()
    }

    fn raw_seq(&self) -> &[u8] {
        self.seq
    }

    fn qual(&self) -> Option<&[u8]> {
        self.qual.or_else(|| self.rec.qual())
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.rec.header_delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.rec.set_header_delim_pos(delim)
    }
}

/// Record that owns all data
/// The header parts are of type `MaybeModified`, since it is necessary to know,
/// whether these were modified in some cases (writing header attributes)
#[derive(Default, Clone)]
pub struct OwnedRecord {
    pub id: MaybeModified<Vec<u8>>,
    pub desc: MaybeModified<Option<Vec<u8>>>,
    pub seq: Vec<u8>,
    pub qual: Option<Vec<u8>>,
}

impl Record for OwnedRecord {
    fn id(&self) -> &[u8] {
        &self.id
    }

    fn desc(&self) -> Option<&[u8]> {
        self.desc.as_deref()
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        (&self.id, self.desc.as_deref())
    }

    fn current_header(&self) -> RecordHeader {
        RecordHeader::IdDesc(
            MaybeModified::new(&self.id, self.id.modified),
            MaybeModified::new(self.desc.as_deref(), self.desc.modified),
        )
    }

    fn raw_seq(&self) -> &[u8] {
        &self.seq
    }

    fn qual(&self) -> Option<&[u8]> {
        self.qual.as_deref()
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
    /// cached = false: the cache will be reset.
    fn get_full<'a>(&'a mut self, rec: &'a dyn Record, cached: bool) -> &'a [u8] {
        if rec.has_seq_lines() {
            if !cached {
                self.0.clear();
                rec.write_seq(&mut self.0);
            }
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

    /// Get the record attribute, the sequence may optionally be cached.
    /// If get_cached = false, the cache will be reset.
    #[inline]
    pub fn get<'a>(
        &'a mut self,
        attr: RecordAttr,
        rec: &'a dyn Record,
        get_cached: bool,
    ) -> &'a [u8] {
        match attr {
            RecordAttr::Id => rec.id(),
            RecordAttr::Desc => rec.desc().unwrap_or(b""),
            RecordAttr::Seq => self.seq_cache.get_full(rec, get_cached),
        }
    }

    #[inline]
    pub fn edit(&mut self, attr: RecordAttr) -> &mut Vec<u8> {
        let v = match attr {
            RecordAttr::Id => self.id.get_or_insert_with(Vec::new),
            RecordAttr::Desc => self.desc.get_or_insert_with(Vec::new),
            RecordAttr::Seq => self.seq.get_or_insert_with(Vec::new),
        };
        v.clear();
        v
    }

    #[inline]
    pub fn edit_with_val<F, O>(
        &mut self,
        attr: RecordAttr,
        rec: &dyn Record,
        get_cached: bool,
        mut func: F,
    ) -> O
    where
        F: FnMut(&[u8], &mut Vec<u8>) -> O,
    {
        match attr {
            RecordAttr::Id => {
                let v = self.id.get_or_insert_with(Vec::new);
                v.clear();
                func(rec.id(), v)
            }
            RecordAttr::Desc => {
                let v = self.desc.get_or_insert_with(Vec::new);
                v.clear();
                func(rec.desc().unwrap_or(b""), v)
            }
            RecordAttr::Seq => {
                let seq = self.seq_cache.get_full(rec, get_cached);
                let v = self.seq.get_or_insert_with(Vec::new);
                v.clear();
                func(seq, v)
            }
        }
    }

    #[inline]
    pub fn record<'r>(&'r self, rec: &'r dyn Record) -> EditedRecord<'r> {
        EditedRecord { editor: self, rec }
    }
}

pub struct EditedRecord<'a> {
    editor: &'a RecordEditor,
    rec: &'a dyn Record,
}

impl Record for EditedRecord<'_> {
    fn id(&self) -> &[u8] {
        self.editor.id.as_deref().unwrap_or_else(|| self.rec.id())
    }

    fn desc(&self) -> Option<&[u8]> {
        self.editor.desc.as_deref().or_else(|| self.rec.desc())
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        if self.editor.id.is_none() && self.editor.desc.is_none() {
            self.rec.id_desc()
        } else {
            (self.id(), self.desc())
        }
    }

    fn current_header(&self) -> RecordHeader {
        if self.editor.id.is_none() && self.editor.desc.is_none() {
            return self.rec.current_header();
        }
        let id = self.id();
        let desc = self.desc();
        RecordHeader::IdDesc(
            MaybeModified::new(id, self.editor.id.is_some()),
            MaybeModified::new(desc, self.editor.desc.is_some()),
        )
    }

    fn raw_seq(&self) -> &[u8] {
        self.editor
            .seq
            .as_deref()
            .unwrap_or_else(|| self.rec.raw_seq())
    }

    fn qual(&self) -> Option<&[u8]> {
        self.rec.qual()
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.rec.header_delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.rec.set_header_delim_pos(delim)
    }

    fn has_seq_lines(&self) -> bool {
        if self.editor.seq.is_some() {
            false
        } else {
            self.rec.has_seq_lines()
        }
    }

    fn seq_segments(&self) -> SeqLineIter {
        self.editor
            .seq
            .as_ref()
            .map(|s| SeqLineIter::OneLine(Some(s)))
            .unwrap_or_else(|| self.rec.seq_segments())
    }
}
