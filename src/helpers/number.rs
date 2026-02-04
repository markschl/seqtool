//! Helpers for number handling

use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use ordered_float::OrderedFloat;

/// Discretizes a floating-point number into bins of width 'interval'
pub fn bin(num: f64, interval: f64) -> Interval {
    let start = (num / interval).floor() * interval;
    Interval::new(start, start + interval)
}

// TODO: consider replacing rust-lexical https://github.com/rustsec/advisory-db/issues/1757
pub fn parse_float(text: &[u8]) -> Result<f64, String> {
    lexical::parse(text).map_err(|_| {
        format!(
            "Could not convert '{}' to a decimal number.",
            String::from_utf8_lossy(text)
        )
    })
}

pub fn parse_int(text: &[u8]) -> Result<i64, String> {
    atoi::atoi(text).ok_or_else(|| {
        format!(
            "Could not convert '{}' to an integer number.",
            String::from_utf8_lossy(text)
        )
    })
}

/// Wrapper used for float values across this crate.
/// It can be sorted/hashed and provides a `Display` implementation that
/// allows to print the numbers in a human-readable way.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[cfg_attr(
    any(feature = "all-commands", feature = "sort", feature = "unique"),
    derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize),
    archive(compare(PartialEq), check_bytes)
)]
pub struct Float(OrderedFloat<f64>);

impl Float {
    pub fn new(f: f64) -> Self {
        Self(OrderedFloat(f))
    }

    pub fn inner(&self) -> f64 {
        self.0.0
    }
}

impl Deref for Float {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Float {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: consider replacing rust-lexical https://github.com/rustsec/advisory-db/issues/1757
        use lexical::WriteFloatOptions;
        let opts = WriteFloatOptions::builder()
            .trim_floats(true)
            .max_significant_digits(std::num::NonZeroUsize::new(6))
            // matching JS formatting
            .nan_string(Some(b"NaN"))
            .inf_string(Some(b"Infinity"))
            .build()
            .unwrap();
        const FMT: u128 = lexical::format::STANDARD;
        let formatted = lexical::to_string_with_options::<_, FMT>(self.inner(), &opts);
        // ryu::Buffer::new().format($f)};
        write!(f, "{formatted}")
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[cfg_attr(
    any(feature = "all-commands", feature = "sort", feature = "unique"),
    derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize),
    archive(compare(PartialEq), check_bytes)
)]
pub struct Interval(pub Float, pub Float);

impl Interval {
    pub fn new(start: f64, end: f64) -> Self {
        Self(Float::new(start), Float::new(end))
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}]", self.0, self.1)
    }
}
