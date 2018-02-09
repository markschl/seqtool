
use std::io;
use std::marker::PhantomData;
use vec_map::VecMap;

use error::CliResult;
use var;

use super::{Record, SeqWriter, Writer, WriteFinish};
use io::DefRecord;

pub struct AttrWriter<W: io::Write, S: SeqWriter<W>> {
    inner: S,
    attrs: Vec<(String, String)>, // used only until 'register_vars' called
    compiled_attrs: VecMap<var::varstring::VarString>,
    temp: (Vec<u8>, Vec<u8>),
    _w: PhantomData<W>,
}

impl<W: io::Write, S: SeqWriter<W>> AttrWriter<W, S> {
    pub fn new(writer: S, attrs: Vec<(String, String)>) -> AttrWriter<W, S> {
        AttrWriter {
            inner: writer,
            attrs: attrs,
            compiled_attrs: VecMap::new(),
            temp: (vec![], vec![]),
            _w: PhantomData
        }
    }
}

impl<W: io::Write, S: SeqWriter<W>> Writer<W> for AttrWriter<W, S> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        for &(ref name, ref value) in &self.attrs {
            let e = var::varstring::VarString::parse_register(value, builder)?;
            let id = builder.register_attr(name, Some(var::attr::Action::Edit));
            self.compiled_attrs.insert(id, e);
        }
        Ok(())
    }

    #[inline]
    fn has_vars(&self) -> bool {
        !self.attrs.is_empty()
    }

    fn write_simple(&mut self, record: &Record) -> CliResult<()> {
        self.inner.write(record)
    }

    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        if vars.attrs().has_attrs() {
            let &mut (ref mut id_out, ref mut desc_out) = &mut self.temp;
            let compiled_attrs = &self.compiled_attrs;
            let (id, desc) = record.id_desc_bytes();
            vars.attrs().compose(id, desc, id_out, desc_out, |id, s| {
                compiled_attrs[id].compose(s, vars.symbols());
            });
            let desc = if desc_out.is_empty() {
                None
            } else {
                Some(desc_out.as_ref())
            };
            self.inner.write(&DefRecord::new(&record, id_out, desc))
        } else {
            self.inner.write(record)
        }
    }

    fn into_inner(self: Box<Self>) -> Option<CliResult<W>> {
        Box::new(self.inner).into_inner()
    }
}
