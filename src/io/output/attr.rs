use std::io;
use std::marker::PhantomData;
use vec_map::VecMap;

use super::{Record, Writer};
use crate::error::CliResult;
use crate::io::{Attribute, DefRecord, SeqWriter};
use crate::var;

pub struct AttrWriter<W: io::Write, S: SeqWriter<W>> {
    inner: S,
    attrs: Vec<Attribute>, // used only until 'register_vars' called
    compiled_attrs: VecMap<var::varstring::VarString>,
    temp: (Vec<u8>, Vec<u8>),
    _w: PhantomData<W>,
}

impl<W: io::Write, S: SeqWriter<W>> AttrWriter<W, S> {
    pub fn new(writer: S, attrs: Vec<Attribute>) -> AttrWriter<W, S> {
        AttrWriter {
            inner: writer,
            attrs,
            compiled_attrs: VecMap::new(),
            temp: (vec![], vec![]),
            _w: PhantomData,
        }
    }
}

impl<W: io::Write, S: SeqWriter<W>> Writer<W> for AttrWriter<W, S> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        for attr in &self.attrs {
            let e = var::varstring::VarString::parse_register(&attr.value, builder)?;
            let id = builder.register_attr(&attr.name, Some(var::attr::Action::Edit));
            self.compiled_attrs.insert(id, e);
        }
        Ok(())
    }

    #[inline]
    fn has_vars(&self) -> bool {
        !self.attrs.is_empty()
    }

    fn write(&mut self, record: &dyn Record, vars: &var::Vars) -> CliResult<()> {
        if vars.attrs().has_attrs() {
            let &mut (ref mut id_out, ref mut desc_out) = &mut self.temp;
            let compiled_attrs = &self.compiled_attrs;
            let (id, desc) = record.id_desc_bytes();
            vars.attrs().compose(id, desc, id_out, desc_out, |id, s| {
                compiled_attrs[id].compose(s, vars.symbols(), record);
            });
            let desc = if desc_out.is_empty() {
                None
            } else {
                Some(desc_out.as_ref())
            };
            self.inner
                .write(&DefRecord::new(&record, id_out, desc), vars)
        } else {
            self.inner.write(record, vars)
        }
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Box::new(self.inner).into_inner()
    }
}
