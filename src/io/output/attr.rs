use vec_map::VecMap;

use error::CliResult;
use var;

use super::{Record, SeqWriter, Writer};

pub struct AttrWriter<W: SeqWriter> {
    inner: W,
    attrs: Vec<(String, String)>, // used only until 'register_vars' called
    compiled_attrs: VecMap<var::varstring::VarString>,
    temp: (Vec<u8>, Vec<u8>),
}

impl<W: SeqWriter> AttrWriter<W> {
    pub fn new(writer: W, attrs: Vec<(String, String)>) -> AttrWriter<W> {
        AttrWriter {
            inner: writer,
            attrs: attrs,
            compiled_attrs: VecMap::new(),
            temp: (vec![], vec![]),
        }
    }
}

impl<W: SeqWriter> Writer for AttrWriter<W> {
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
        self.inner
            .write(record.id_bytes(), record.desc_bytes(), record)
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
            self.inner.write(id_out, desc, record)
        } else {
            self.inner
                .write(record.id_bytes(), record.desc_bytes(), record)
        }
    }
}
