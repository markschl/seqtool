use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::mem::size_of_val;

use rkyv::{Archive, Deserialize, Serialize};

use crate::helpers::value::SimpleValue;

pub fn item_size(key: &SimpleValue, record: &Vec<u8>) -> usize {
    key.size() + size_of_val(record) + size_of_val(&**record)
}

/// Item used in sort and unique commands:
/// holds a key and a formatted record
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item {
    pub key: SimpleValue,
    pub record: Vec<u8>,
}

impl Item {
    pub fn new(key: SimpleValue, record: Vec<u8>) -> Self {
        Self { key, record }
    }

    pub fn size(&self) -> usize {
        item_size(&self.key, &self.record)
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

impl Hash for Item {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}
