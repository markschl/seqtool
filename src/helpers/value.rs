use std::mem::size_of_val;

use ordered_float::OrderedFloat;
use rkyv::{Archive, Deserialize, Serialize};

/// A simple value type that can be either text, numeric or none.
/// Can also be serialized using rkyv.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq), check_bytes)]
pub enum SimpleValue {
    Text(Vec<u8>),
    Number(OrderedFloat<f64>),
    None,
}

impl SimpleValue {
    pub fn size(&self) -> usize {
        size_of_val(self)
            + match self {
                SimpleValue::Text(v) => size_of_val(&**v),
                _ => 0,
            }
    }
}
