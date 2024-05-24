use std::io;
use std::mem;

use deepsize::{Context, DeepSizeOf};

use crate::io::Record;
use crate::var::symbols::{OptValue, Value};

use super::number::{Float, Interval};

/// A simple value type that can be either text, numeric, boolean, interval or undefined/none.
/// Can also be serialized using rkyv (only enabled for sort and unique commands).
///
/// This type is simpler than the Value type in the symbol table, which often have
/// additional information stored/allocated.
/// Another difference: SimpleValue does not have an integer type, any number will
/// thus behave the same (as float). This is important when sorting/hashing.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(
    any(feature = "all-commands", feature = "sort", feature = "unique"),
    derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize),
    archive(compare(PartialEq), check_bytes)
)]
pub enum SimpleValue {
    Text(Box<[u8]>),
    Number(Float),
    Boolean(bool),
    Interval(Interval),
    None,
}

impl SimpleValue {
    #[inline]
    pub fn write<W: io::Write + ?Sized>(&self, writer: &mut W, none_value: &str) -> io::Result<()> {
        use SimpleValue::*;
        match self {
            Text(v) => writer.write_all(v),
            Number(v) => write!(writer, "{}", v),
            Boolean(v) => write!(writer, "{}", v),
            Interval(i) => write!(writer, "{}", i),
            None => write!(writer, "{}", none_value),
        }
    }

    #[inline]
    pub fn to_symbol(&self, sym: &mut OptValue) {
        use SimpleValue::*;
        match self {
            Text(t) => sym.inner_mut().set_text(t),
            Number(n) => sym.inner_mut().set_float(n.inner()),
            Boolean(b) => sym.inner_mut().set_bool(*b),
            Interval(i) => sym.inner_mut().set_interval(*i),
            None => sym.set_none(),
        }
    }

    #[inline]
    pub fn replace_from_symbol(
        &mut self,
        sym: &OptValue,
        rec: &dyn Record,
        text_buf: &mut Vec<u8>,
    ) {
        if let SimpleValue::Text(t) = self {
            // If present, take the text buffer from SimpleValue.
            // If `text_buf` is already non-empty (allocated), this allocation
            // will be lost. But it is assumed that the allocation is always
            // either referenced by SimpleValue::Text() or by `text_buf`, never
            // both.
            *text_buf = mem::take(t).into_vec();
        }
        *self = if let Some(v) = sym.inner() {
            match v {
                Value::Text(_) | Value::Attr(_) => {
                    v.as_text(rec, |t| {
                        text_buf.clear();
                        text_buf.extend_from_slice(t);
                        Ok::<(), ()>(())
                    })
                    .unwrap();
                    SimpleValue::Text(mem::take(text_buf).into_boxed_slice())
                }
                Value::Int(v) => SimpleValue::Number(Float::new(*v.get() as f64)),
                Value::Float(v) => SimpleValue::Number(Float::new(*v.get())),
                Value::Interval(v) => SimpleValue::Interval(*v.get()),
                Value::Bool(v) => SimpleValue::Boolean(*v.get()),
            }
        } else {
            SimpleValue::None
        };
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
