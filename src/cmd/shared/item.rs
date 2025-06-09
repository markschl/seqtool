use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use deepsize::DeepSizeOf;
use rkyv::{Archive, Deserialize, Serialize};

use crate::helpers::{value::SimpleValue, write_list::write_list_with};
use crate::io::Record;
use crate::var::{
    symbols::{OptValue, SymbolTable},
    varstring::VarString,
};

use super::tmp_store::Archivable;

#[derive(
    DeepSizeOf,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
)]
#[archive(compare(PartialEq), check_bytes)]
pub enum Key {
    Single(SimpleValue),
    // This saves time with two values per key (but appears to make >2 values slower)
    // TODO: activating this comes with the tradeoff of increased memory usage
    // Two([SimpleValue; 2]),
    Multiple(Box<[SimpleValue]>),
}

impl Key {
    pub fn with_size(key_size: usize) -> Self {
        match key_size {
            0 => panic!(),
            1 => Self::Single(SimpleValue::None),
            // 2 => Self::Two([SimpleValue::None, SimpleValue::None]),
            _ => Self::Multiple(vec![SimpleValue::None; key_size].into_boxed_slice()),
        }
    }

    pub fn as_slice(&self) -> &[SimpleValue] {
        match self {
            Self::Single(v) => std::slice::from_ref(v),
            // Self::Two(v) => v,
            Self::Multiple(v) => v,
        }
    }

    pub fn compose_from(
        &mut self,
        varstrings: &[VarString],
        key_buf: &mut [Vec<u8>],
        symbols: &SymbolTable,
        record: &dyn Record,
    ) -> Result<(), String> {
        match self {
            Key::Single(v) => {
                debug_assert!(varstrings.len() == 1 && key_buf.len() == 1);
                varstrings[0].simple_value(v, &mut key_buf[0], symbols, record)?
            }
            // Key::Two(v) => {
            //     debug_assert!(varstrings.len() == 2 && key_buf.len() == 2);
            //     for i in 0..2 {
            //         varstrings[i].into_simple(
            //             &mut v[i],
            //             &mut key_buf[i],
            //             symbols,
            //             record,
            //             force_numeric,
            //         )?;
            //     }
            // }
            Key::Multiple(values) => {
                debug_assert!(varstrings.len() == values.len() && key_buf.len() == values.len());
                for ((vs, key_buf), val) in varstrings
                    .iter()
                    .zip(key_buf.iter_mut())
                    .zip(values.iter_mut())
                {
                    vs.simple_value(val, key_buf, symbols, record)?;
                }
            }
        }
        Ok(())
    }

    pub fn write_to_symbol(&self, sym: &mut OptValue) {
        match self {
            Key::Single(v) => v.to_symbol(sym),
            // Key::Two(v) => {
            //     let text = sym.inner_mut().mut_text();
            //     write_list_with(v, b",", text, |v, o| v.write(o)).unwrap();
            // }
            Key::Multiple(values) => {
                let text = sym.inner_mut().mut_text();
                write_list_with(values.iter(), b",", text, |v, o| v.write(o)).unwrap();
            }
        }
    }
}

// for error messages
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, k) in self.as_slice().iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", k)?;
        }
        Ok(())
    }
}

impl Deref for Key {
    type Target = [SimpleValue];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Single(v) => std::slice::from_ref(v),
            Self::Multiple(v) => v,
        }
    }
}

impl DerefMut for Key {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Single(v) => std::slice::from_mut(v),
            Self::Multiple(v) => v,
        }
    }
}

/// Item used in sort and unique commands:
/// holds a key and a formatted record,
/// but only the key is used for comparisons.
#[derive(Archive, Deserialize, Serialize, DeepSizeOf, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item<R: for<'a> Archivable<'a> + DeepSizeOf> {
    pub key: Key,
    pub record: R,
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Item<R> {
    pub fn new(key: Key, record: R) -> Self {
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
