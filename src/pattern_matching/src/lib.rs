
pub type Text = Vec<u8>;
/// Type alias for a text slice, i.e. ``&[u8]``.
pub type TextSlice<'a> = &'a [u8];

/// Type alias for an iterator over a sequence, i.e. ``Iterator<Item=&u8>``.
pub trait TextIterator<'a>: Iterator<Item = &'a u8> {}
impl<'a, I: Iterator<Item = &'a u8>> TextIterator<'a> for I {}

/// Type alias for a type that can be coerced into a `TextIterator`.
/// This includes ``&Vec<u8>``, ``&[u8]``, ``Iterator<Item=&u8>``.
pub trait IntoTextIterator<'a>: IntoIterator<Item = &'a u8> {}
impl<'a, T: IntoIterator<Item = &'a u8>> IntoTextIterator<'a> for T {}


pub mod myers;
