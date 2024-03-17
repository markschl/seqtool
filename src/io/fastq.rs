use std::io;

use seq_io::fastq::{self, Reader, Record as FR};
use seq_io::policy::BufPolicy;

use crate::config::SeqContext;
use crate::error::CliResult;
use crate::var::VarBuilder;

use super::fastx::FastxHeaderParser;
use super::output::{attr::register_attributes, FormatWriter};
use super::{Attribute, MaybeModified, QualFormat, Record, RecordHeader, SeqReader};

// Reader

pub struct FastqReader<R: io::Read, P: BufPolicy>(pub Reader<R, P>);

impl<R, P> FastqReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    pub fn new(rdr: R, cap: usize, policy: P) -> Self {
        FastqReader(Reader::with_capacity(rdr, cap).set_policy(policy))
    }
}

impl<R, P, O> SeqReader<O> for FastqReader<R, P>
where
    R: io::Read,
    P: BufPolicy,
{
    fn read_next(&mut self, func: &mut dyn FnMut(&dyn Record) -> O) -> Option<CliResult<O>> {
        self.0.next().map(|r| {
            let r = FastqRecord::new(r?);
            Ok(func(&r))
        })
    }
}

// Wrapper for FASTQ record

pub struct FastqRecord<'a> {
    rec: fastq::RefRecord<'a>,
    header_parser: FastxHeaderParser,
}

impl<'a> FastqRecord<'a> {
    #[inline(always)]
    pub fn new(inner: fastq::RefRecord<'a>) -> FastqRecord<'a> {
        FastqRecord {
            rec: inner,
            header_parser: Default::default(),
        }
    }
}

impl<'a> Record for FastqRecord<'a> {
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
        Some(<fastq::RefRecord as fastq::Record>::qual(&self.rec))
    }

    fn header_delim_pos(&self) -> Option<Option<usize>> {
        self.header_parser.delim_pos()
    }

    fn set_header_delim_pos(&self, delim: Option<usize>) {
        self.header_parser.set_delim_pos(Some(delim))
    }
}

// Writer

pub struct FastqWriter {
    format: QualFormat,
}

impl FastqWriter {
    pub fn new(
        format: QualFormat,
        attrs: &[(Attribute, bool)],
        builder: &mut VarBuilder,
    ) -> CliResult<Self> {
        register_attributes(attrs, builder)?;
        Ok(Self { format })
    }
}

impl FormatWriter for FastqWriter {
    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        write_fastq(record, out, ctx, self.format)
    }
}

fn write_fastq<W: io::Write>(
    record: &dyn Record,
    mut out: W,
    ctx: &mut SeqContext,
    format: QualFormat,
) -> CliResult<()> {
    // TODO: could use seq_io::fastq::write_to / write_parts, but the sequence is an iterator of segments

    // header
    out.write_all(b"@")?;
    ctx.attrs.write_head(record, &mut out, &ctx.symbols)?;
    out.write_all(b"\n")?;

    // sequence
    for seq in record.seq_segments() {
        out.write_all(seq)?;
    }
    out.write_all(b"\n+\n")?;

    // quality scores
    let qual = record.qual().ok_or("No quality scores found in input.")?;
    let qual = ctx.qual_converter.convert_to(qual, format).map_err(|e| {
        format!(
            "Error writing record '{}'. {}",
            String::from_utf8_lossy(record.id()),
            e
        )
    })?;
    out.write_all(qual)?;
    out.write_all(b"\n")?;

    Ok(())
}
