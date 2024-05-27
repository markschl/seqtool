//use std::ops::Deref;
use std::{convert::AsRef, io};

/// Writes an iterator of of text slices as delimited list to the output.
/// Returns true if the list is not empty
pub fn write_list<L, I, W>(list: L, sep: &[u8], out: &mut W) -> io::Result<bool>
where
    L: IntoIterator<Item = I>,
    I: AsRef<[u8]>,
    W: io::Write + ?Sized,
{
    write_list_with(list, sep, out, |item, o| o.write_all(item.as_ref()))
}

/// Writes an iterator of of values as delimited list to the output
/// using a custom writing function.
/// Returns true if the list is not empty
#[inline]
pub fn write_list_with<L, I, W, F>(
    list: L,
    sep: &[u8],
    out: &mut W,
    mut write_fn: F,
) -> io::Result<bool>
where
    L: IntoIterator<Item = I>,
    W: io::Write + ?Sized,
    F: FnMut(I, &mut W) -> io::Result<()>,
{
    let mut first = true;
    for item in list {
        if first {
            first = false;
        } else {
            out.write_all(sep)?;
        }
        write_fn(item, out)?;
    }
    Ok(!first)
}
