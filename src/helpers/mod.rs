//! Utilities used by many commands,
//! which do not use optional crates that depend on feature flags.

use std::collections::{HashMap, HashSet};

// The default hash map to use
use ahash::RandomState;

pub type DefaultHashMap<K, V> = HashMap<K, V, RandomState>;
pub type DefaultHashSet<V> = HashSet<V, RandomState>;
pub type DefaultBuildHasher = ahash::RandomState; // BuildHasherDefault<ahash::AHasher>;

#[macro_use]
pub mod macros;
pub mod any;
pub mod bytesize;
pub mod heap_merge;
pub mod rng;
pub mod util;
pub mod value;
pub mod var_range;
pub mod vec;
