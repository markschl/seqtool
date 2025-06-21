//! This module contains code shared between at least two commands, which
//! relies on some external crates and therefore needs feature flags.

#[cfg(any(
    feature = "all-commands",
    feature = "cmp",
    feature = "count",
    feature = "sort",
    feature = "unique"
))]
pub mod key;

#[cfg(any(feature = "all-commands", feature = "sort", feature = "unique"))]
pub mod tmp_store;
