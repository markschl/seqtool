pub mod pass;
#[cfg(feature = "view")]
pub mod view;

pub mod count;
pub mod stat;

#[cfg(feature = "expr")]
pub mod filter;
pub mod head;
pub mod interleave;
pub mod sample;
pub mod slice;
pub mod sort;
pub mod split;
pub mod tail;
pub mod unique;

#[cfg(feature = "find")]
pub mod find;
pub mod replace;

pub mod concat;
pub mod del;
pub mod lower;
pub mod mask;
pub mod revcomp;
pub mod set;
pub mod trim;
pub mod upper;
