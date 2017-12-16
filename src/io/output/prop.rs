use vec_map::VecMap;

use error::CliResult;
use var;

use super::{Record, SeqWriter, Writer};

pub struct PropWriter<W: SeqWriter> {
    inner: W,
    props: Vec<(String, String)>, // used only until 'register_vars' called
    compiled_props: VecMap<var::varstring::VarString>,
    temp: (Vec<u8>, Vec<u8>),
}

impl<W: SeqWriter> PropWriter<W> {
    pub fn new(writer: W, props: Vec<(String, String)>) -> PropWriter<W> {
        PropWriter {
            inner: writer,
            props: props,
            compiled_props: VecMap::new(),
            temp: (vec![], vec![]),
        }
    }
}

impl<W: SeqWriter> Writer for PropWriter<W> {
    fn register_vars(&mut self, builder: &mut var::VarBuilder) -> CliResult<()> {
        for &(ref name, ref value) in &self.props {
            let e = var::varstring::VarString::parse_register(value, builder)?;
            let id = builder.register_prop(name, Some(var::prop::Action::Edit));
            self.compiled_props.insert(id, e);
        }
        Ok(())
    }

    #[inline]
    fn has_vars(&self) -> bool {
        !self.props.is_empty()
    }

    fn write_simple(&mut self, record: &Record) -> CliResult<()> {
        self.inner
            .write(record.id_bytes(), record.desc_bytes(), record)
    }

    fn write(&mut self, record: &Record, vars: &var::Vars) -> CliResult<()> {
        if vars.props().has_props() {
            let &mut (ref mut id_out, ref mut desc_out) = &mut self.temp;
            let compiled_props = &self.compiled_props;
            let (id, desc) = record.id_desc_bytes();
            vars.props().compose(id, desc, id_out, desc_out, |id, s| {
                compiled_props[id].compose(s, vars.symbols());
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
