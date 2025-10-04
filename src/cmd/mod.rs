pub mod shared;

#[cfg(any(feature = "all-commands", feature = "hist"))]
pub mod hist;
#[cfg(any(feature = "all-commands", feature = "pass", feature = "stat"))]
pub mod pass;
#[cfg(any(feature = "all-commands", feature = "view"))]
pub mod view;

#[cfg(any(feature = "all-commands", feature = "count"))]
pub mod count;
#[cfg(any(feature = "all-commands", feature = "stat"))]
pub mod stat;

#[cfg(any(feature = "all-commands", feature = "cmp"))]
pub mod cmp;
#[cfg(any(
    all(feature = "expr", feature = "all-commands"),
    all(feature = "expr", feature = "filter")
))]
pub mod filter;
#[cfg(any(feature = "all-commands", feature = "head"))]
pub mod head;
#[cfg(any(feature = "all-commands", feature = "interleave"))]
pub mod interleave;
#[cfg(any(feature = "all-commands", feature = "sample"))]
pub mod sample;
#[cfg(any(feature = "all-commands", feature = "slice"))]
pub mod slice;
#[cfg(any(feature = "all-commands", feature = "sort"))]
pub mod sort;
#[cfg(any(feature = "all-commands", feature = "split"))]
pub mod split;
#[cfg(any(feature = "all-commands", feature = "tail"))]
pub mod tail;
#[cfg(any(feature = "all-commands", feature = "unique"))]
pub mod unique;

#[cfg(any(feature = "all-commands", feature = "concat"))]
pub mod concat;
#[cfg(any(feature = "all-commands", feature = "del"))]
pub mod del;
#[cfg(any(feature = "all-commands", feature = "find"))]
pub mod find;
#[cfg(any(feature = "all-commands", feature = "lower"))]
pub mod lower;
#[cfg(any(feature = "all-commands", feature = "mask"))]
pub mod mask;
#[cfg(any(feature = "all-commands", feature = "replace"))]
pub mod replace;
#[cfg(any(feature = "all-commands", feature = "revcomp"))]
pub mod revcomp;
#[cfg(any(feature = "all-commands", feature = "set"))]
pub mod set;
#[cfg(any(feature = "all-commands", feature = "trim"))]
pub mod trim;
#[cfg(any(feature = "all-commands", feature = "upper"))]
pub mod upper;
