use std::io;

use seq_io::fasta::{self, Reader, Record as SeqioRecord};
use seq_io::policy::BufPolicy;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::var::VarBuilder;

use super::fastx::FastxHeaderParser;
use super::output::{attr::register_attributes, FormatWriter};
use super::{Attribute, MaybeModified, Record, RecordHeader, SeqLineIter, SeqReader};

// Reader

pub struct FastaReader<R: io::Read, P: BufPolicy>(pub Reader<R, P>);

impl<R, P> FastaReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    pub fn new(rdr: R, cap: usize, policy: P) -> Self {
        FastaReader(Reader::with_capacity(rdr, cap).set_policy(policy))
    }
}

impl<R, P> SeqReader for FastaReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(
        &mut self,
        func: &mut dyn FnMut(&dyn Record) -> CliResult<bool>,
    ) -> Option<CliResult<bool>> {
        self.0.next().map(|r| {
            let r = FastaRecord::new(r?);
            func(&r)
        })
    }
}

// Wrapper for FASTA record

pub struct FastaRecord<'a> {
    rec: fasta::RefRecord<'a>,
    header_parser: FastxHeaderParser,
}

impl<'a> FastaRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fasta::RefRecord<'a>) -> FastaRecord<'a> {
        FastaRecord {
            rec: inner,
            header_parser: Default::default(),
        }
    }
}

impl Record for FastaRecord<'_> {
    fn id(&self) -> &[u8] {
        self.header_parser.id_desc(self.rec.head()).0
    }

    fn desc(&self) -> Option<&[u8]> {
        self.header_parser.id_desc(self.rec.head()).1
    }

    fn id_desc(&self) -> (&[u8], Option<&[u8]>) {
        self.header_parser.id_desc(self.rec.head())
    }

    fn current_header(&self) -> RecordHeader {
        if let Some((id, desc)) = self.header_parser.parsed_id_desc(self.rec.head()) {
            RecordHeader::IdDesc(
                MaybeModified::new(id, false),
                MaybeModified::new(desc, false),
            )
        } else {
            RecordHeader::Full(self.rec.head())
        }
    }

    fn raw_seq(&self) -> &[u8] {
        self.rec.seq()
    }

    fn qual(&self) -> Option<&[u8]> {
        None
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.header_parser.delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.header_parser.set_delim_pos(Some(delim))
    }

    fn has_seq_lines(&self) -> bool {
        self.rec.num_seq_lines() > 1
    }

    fn seq_segments(&self) -> SeqLineIter {
        SeqLineIter::Fasta(self.rec.seq_lines())
    }
}

// Writer

pub struct FastaWriter {
    wrap: Option<usize>,
}

impl FastaWriter {
    pub fn new(
        wrap: Option<usize>,
        attrs: &[(Attribute, bool)],
        builder: &mut VarBuilder,
    ) -> CliResult<Self> {
        register_attributes(attrs, builder)?;
        Ok(Self { wrap })
    }
}

impl FormatWriter for FastaWriter {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        write_fasta(record, out, ctx, self.wrap)
    }
}

fn write_fasta<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    ctx: &mut SeqContext,
    wrap: Option<usize>,
) -> CliResult<()> {
    out.write_all(b">")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;
    if let Some(w) = wrap {
        fasta::write_wrap_seq_iter(&mut out, record.seq_segments(), w)?;
    } else {
        fasta::write_seq_iter(&mut out, record.seq_segments())?;
    }
    Ok(())
}
