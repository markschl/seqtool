
use std::str::{self, Utf8Error};
use std::ascii::AsciiExt;

use seq_io::fasta;

pub trait Record {
    //type SeqSegments: Iterator<Item=&'a [u8]> + 'a;
    fn id_bytes(&self) -> &[u8];
    fn desc_bytes(&self) -> Option<&[u8]>;
    fn raw_seq(&self) -> &[u8];
    fn qual(&self) -> Option<&[u8]>;

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

    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Other(Some(self.raw_seq()))
    }

    fn seq_len(&self) -> usize {
        self.seq_segments().fold(0, |l, s| l + s.len())
    }

    fn write_seq(&self, to: &mut Vec<u8>) {
        for seq in self.seq_segments() {
            to.extend_from_slice(seq);
        }
    }

    fn write_attr(&self, attr: Attribute, out: &mut Vec<u8>) {
        match attr {
            Attribute::Id => {
                out.extend_from_slice(self.id_bytes());
            }
            Attribute::Desc => {
                self.desc_bytes().map(|d| out.extend_from_slice(d));
            }
            Attribute::Seq => for s in self.seq_segments() {
                out.extend_from_slice(s);
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Attribute {
    Id,
    Desc,
    Seq,
    //Qual,
}

impl Attribute {
    pub fn from_str(attr: &str) -> Option<Attribute> {
        Some(if attr.eq_ignore_ascii_case("id") {
            Attribute::Id
        } else if attr.eq_ignore_ascii_case("desc") {
            Attribute::Desc
        } else if attr.eq_ignore_ascii_case("seq") {
            Attribute::Seq
        }
        //else if attr.eq_ignore_ascii_case("qual") { Attribute::Qual }
        else {
            return None;
        })
    }
}

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
    fn raw_seq(&self) -> &[u8] {
        (**self).raw_seq()
    }
    fn qual(&self) -> Option<&[u8]> {
        (**self).qual()
    }
    fn write_seq(&self, to: &mut Vec<u8>) {
        (**self).write_seq(to)
    }
    fn seq_segments(&self) -> SeqLineIter {
        (**self).seq_segments()
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
            id: id,
            desc: desc,
        }
    }
}

impl<'a> DefRecord<'a, &'a Record> {
    pub fn from_rec(inner: &'a Record) -> DefRecord<'a, &'a Record> {
        let (id, desc) = inner.id_desc_bytes();
        DefRecord::new(inner, id, desc)
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
    fn raw_seq(&self) -> &[u8] {
        self.rec.raw_seq()
    }
    fn qual(&self) -> Option<&[u8]> {
        self.rec.qual()
    }
    fn write_seq(&self, to: &mut Vec<u8>) {
        self.rec.write_seq(to)
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
            seq: seq,
            qual: qual,
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
    fn raw_seq(&self) -> &[u8] {
        self.seq
    }
    fn qual(&self) -> Option<&[u8]> {
        self.qual.or_else(|| self.rec.qual())
    }
    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Other(Some(self.seq))
    }
}

// Wrapper for editing any attribute

#[derive(Debug, Default)]
pub struct RecordEditor {
    id: Option<Vec<u8>>,
    desc: Option<Vec<u8>>,
    seq: Option<Vec<u8>>,
    seq_cache: Vec<u8>,
    //qual: Option<Vec<u8>>,
}

impl RecordEditor {
    pub fn new() -> RecordEditor {
        RecordEditor {
            id: None,
            desc: None,
            seq: None,
            seq_cache: vec![],
        }
    }

    #[inline]
    pub fn get<'a>(&'a mut self, attr: Attribute, rec: &'a Record, cached: bool) -> &'a [u8] {
        match attr {
            Attribute::Id => rec.id_bytes(),
            Attribute::Desc => rec.desc_bytes().unwrap_or(b""),
            Attribute::Seq => {
                if !cached {
                    self.cache_seq(rec);
                }
                &self.seq_cache
            }
        }
    }

    #[inline]
    fn cache_seq(&mut self, rec: &Record) {
        self.seq_cache.clear();
        for seq in rec.seq_segments() {
            self.seq_cache.extend_from_slice(seq);
        }
    }

    #[inline]
    pub fn edit(&mut self, attr: Attribute) -> &mut Vec<u8> {
        let v = match attr {
            Attribute::Id => self.id.get_or_insert_with(|| vec![]),
            Attribute::Desc => self.desc.get_or_insert_with(|| vec![]),
            Attribute::Seq => self.seq.get_or_insert_with(|| vec![]),
        };
        v.clear();
        v
    }

    #[inline]
    pub fn edit_with_val<F, O>(
        &mut self,
        attr: Attribute,
        rec: &Record,
        cached: bool,
        mut func: F,
    ) -> O
    where
        F: FnMut(&[u8], &mut Vec<u8>) -> O,
    {
        match attr {
            Attribute::Id => {
                let v = self.id.get_or_insert_with(|| vec![]);
                v.clear();
                func(rec.id_bytes(), v)
            }
            Attribute::Desc => {
                let v = self.desc.get_or_insert_with(|| vec![]);
                v.clear();
                func(rec.desc_bytes().unwrap_or(b""), v)
            }
            Attribute::Seq => {
                if ! cached {
                    self.cache_seq(rec);
                }
                let v = self.seq.get_or_insert_with(|| vec![]);
                v.clear();
                func(&self.seq_cache, v)
            }
            //Attribute::Qual => &mut self.qual,
        }
    }

    #[inline]
    pub fn rec<'r>(&'r self, rec: &'r Record) -> EditedRecord<'r> {
        EditedRecord {
            editor: self,
            rec: rec,
        }
    }
}

pub struct EditedRecord<'a> {
    editor: &'a RecordEditor,
    rec: &'a Record,
}

impl<'r> Record for EditedRecord<'r> {
    fn id_bytes(&self) -> &[u8] {
        self.editor
            .id
            .as_ref()
            .map(|i| i.as_slice())
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
    fn raw_seq(&self) -> &[u8] {
        self.editor
            .seq
            .as_ref()
            .map(|s| s.as_slice())
            .unwrap_or_else(|| self.rec.raw_seq())
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

    fn write_seq(&self, to: &mut Vec<u8>) {
        for seq in self.seq_segments() {
            to.extend_from_slice(seq);
        }
    }
}
