use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use deepsize::DeepSizeOf;
use rkyv::{Archive, Deserialize, Serialize};

use crate::helpers::value::SimpleValue;

use super::tmp_store::Archivable;

/// Item used in sort and unique commands:
/// holds a key and a formatted record,
/// but only the key is used for comparisons.
#[derive(Archive, Deserialize, Serialize, DeepSizeOf, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item<R: for<'a> Archivable<'a> + DeepSizeOf> {
    pub key: SimpleValue,
    pub record: R,
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Item<R> {
    pub fn new(key: SimpleValue, record: R) -> Self {
        Self { key, record }
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> PartialOrd for Item<R> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> PartialEq for Item<R> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Eq for Item<R> {}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Ord for Item<R> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Hash for Item<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Archivable<'_> for Item<R> {}
