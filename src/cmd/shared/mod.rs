//! This module contains code shared between at least two commands, which
//! relies on some external crates and therefore needs feature flags.

#[cfg(any(feature = "all-commands", feature = "find", feature = "view"))]
pub mod seqtype;
cfg_if::cfg_if! { if #[cfg(any(feature = "all-commands", feature = "sort", feature = "unique"))] {
    pub mod tmp_store;
    pub mod sort_item;
    pub mod key_var;
}}
