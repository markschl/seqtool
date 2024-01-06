use std::io;
use vec_map::VecMap;

use super::{FormatWriter, Record};
use crate::config::SeqContext;
use crate::error::CliResult;
use crate::io::{Attribute, DefRecord, SeqWriter};
use crate::var;

pub struct AttrWriter<S: SeqWriter> {
    inner: S,
    registered_attrs: VecMap<var::varstring::VarString>,
    temp: (Vec<u8>, Vec<u8>),
}

impl<S: SeqWriter> AttrWriter<S> {
    pub fn new(writer: S, attrs: &[Attribute], builder: &mut var::VarBuilder) -> CliResult<Self> {
        let mut registered_attrs = VecMap::new();
        for attr in attrs {
            let (e, _) = var::varstring::VarString::parse_register(&attr.value, builder)?;
            let id = builder.register_attr(&attr.name, Some(var::attr::Action::Edit));
            registered_attrs.insert(id, e);
        }
        Ok(Self {
            inner: writer,
            registered_attrs,
            temp: (vec![], vec![]),
        })
    }
}

impl<S: SeqWriter> FormatWriter for AttrWriter<S> {
    #[inline]
    fn has_vars(&self) -> bool {
        !self.registered_attrs.is_empty()
    }

    fn write(
        &mut self,
        record: &dyn Record,
        out: &mut dyn io::Write,
        ctx: &mut SeqContext,
    ) -> CliResult<()> {
        if ctx.attrs.has_attrs() {
            let &mut (ref mut rec_id_out, ref mut rec_desc_out) = &mut self.temp;
            let registered_attrs = &self.registered_attrs;
            let (rec_id, rec_desc) = record.id_desc_bytes();
            ctx.attrs
                .compose(rec_id, rec_desc, rec_id_out, rec_desc_out, |attr_id, s| {
                    registered_attrs[attr_id].compose(s, &mut ctx.symbols, record)
                })?;
            let _rec_desc_out = if rec_desc_out.is_empty() {
                None
            } else {
                Some(rec_desc_out.as_ref())
            };
            self.inner.write(
                &DefRecord::new(&record, rec_id_out, _rec_desc_out),
                ctx,
                out,
            )
        } else {
            self.inner.write(record, ctx, out)
        }
    }
}
