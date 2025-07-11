//! Utilities used by many commands,
//! which do not use optional crates that depend on feature flags.

use std::collections::{HashMap, HashSet};

// The default hash map to use
use ahash::RandomState;

pub type DefaultHashMap<K, V> = HashMap<K, V, RandomState>;
pub type DefaultHashSet<V> = HashSet<V, RandomState>;
pub type DefaultBuildHasher = ahash::RandomState; // BuildHasherDefault<ahash::AHasher>;

// missing data string
pub const NA: &str = "undefined";

#[macro_use]
pub mod macros;
pub mod any;
pub mod bytesize;
pub mod complement;
pub mod heap_merge;
pub mod number;
pub mod replace;
pub mod rng;
pub mod seqtype;
pub mod slice;
pub mod thread_local;
pub mod value;
pub mod var_range;
pub mod vec_buf;
pub mod write_list;
