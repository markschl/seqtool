use std::cmp::Ordering;
use std::mem::size_of_val;

use ordered_float::OrderedFloat;
use rkyv::{Archive, Deserialize, Serialize};

use crate::var::varstring::DynValue;

// #[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Hash, Clone, Archive, Serialize, Deserialize)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq), check_bytes)]
pub enum Key {
    Text(Vec<u8>),
    Numeric(OrderedFloat<f64>),
    None,
}

impl Key {
    pub fn size(&self) -> usize {
        match self {
            Key::Text(v) => size_of_val(v) + size_of_val(&**v),
            _ => size_of_val(self),
        }
    }
}

impl<'a> From<Option<DynValue<'a>>> for Key {
    fn from(v: Option<DynValue<'a>>) -> Self {
        match v {
            Some(DynValue::Text(v)) => Key::Text(v.to_vec()),
            Some(DynValue::Numeric(v)) => Key::Numeric(OrderedFloat(v)),
            None => Key::None,
        }
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item {
    pub key: Key,
    pub record: Vec<u8>,
}

impl Item {
    pub fn new(key: Key, record: Vec<u8>) -> Self {
        Self { key, record }
    }

    pub fn size(&self) -> usize {
        self.key.size() + size_of_val(&self.record) + size_of_val(&*self.record)
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Item {}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}
