use std::mem::size_of_val;

use ordered_float::OrderedFloat;

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
