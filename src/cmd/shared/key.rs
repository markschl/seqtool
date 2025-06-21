use std::fmt;
use std::ops::{Deref, DerefMut};

use deepsize::DeepSizeOf;

use crate::helpers::{value::SimpleValue, write_list::write_list_with};
use crate::io::Record;
use crate::var::{
    symbols::{OptValue, SymbolTable},
    varstring::VarString,
};

#[derive(DeepSizeOf, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(
    any(feature = "all-commands", feature = "sort", feature = "unique"),
    derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize),
    archive(compare(PartialEq), check_bytes)
)]
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
