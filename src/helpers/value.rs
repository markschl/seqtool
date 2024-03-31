use std::io;

use deepsize::{Context, DeepSizeOf};
use ordered_float::OrderedFloat;

use crate::{cmd::shared::tmp_store::Archivable, var::symbols::OptValue};

/// A simple value type that can be either text, numeric or none.
/// Can also be serialized using rkyv (only enabled for sort and unique commands).
// TODO: may belong in cmd::shared, but SimpleValue is also used in VarString::get_simple()
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(
    any(feature = "all-commands", feature = "sort", feature = "unique"),
    derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize),
    archive(compare(PartialEq), check_bytes)
)]
pub enum SimpleValue {
    Text(Box<[u8]>),
    Number(OrderedFloat<f64>),
    None,
}

impl SimpleValue {
    pub fn write(&self, writer: &mut impl io::Write) -> io::Result<()> {
        match self {
            SimpleValue::Text(v) => writer.write_all(v),
            SimpleValue::Number(v) => write!(writer, "{}", v),
            SimpleValue::None => Ok(()),
        }
    }

    pub fn into_symbol(&self, sym: &mut OptValue) {
        match self {
            SimpleValue::Text(t) => sym.inner_mut().set_text(t),
            SimpleValue::Number(n) => sym.inner_mut().set_float(n.0),
            SimpleValue::None => sym.set_none(),
        }
    }
}

impl DeepSizeOf for SimpleValue {
    fn deep_size_of_children(&self, _: &mut Context) -> usize {
        if let SimpleValue::Text(v) = self {
            return v.len();
        }
        0
    }
}

impl<'a> Archivable<'a> for SimpleValue {}
