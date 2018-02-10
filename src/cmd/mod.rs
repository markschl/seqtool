pub mod pass;
pub mod count;

pub mod slice;
pub mod sample;
pub mod head;
pub mod tail;

pub mod trim;
pub mod set;
pub mod del;
pub mod replace;
pub mod find;
pub mod split;
pub mod upper;
pub mod lower;
pub mod mask;
pub mod revcomp;
pub mod stat;
#[cfg(feature = "exprtk")]
pub mod filter;
pub mod interleave;
