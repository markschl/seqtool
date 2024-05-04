//use std::ops::Deref;
use std::{convert::AsRef, io};

use crate::helpers::DefaultHashMap as HashMap;

pub fn match_fields<'a, S1, S2>(fields: &'a [S1], other: &'a [S2]) -> Result<Vec<usize>, &'a str>
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let other: HashMap<_, _> = other
        .iter()
        .enumerate()
        .map(|(i, f)| (f.as_ref(), i))
        .collect();

    fields
        .iter()
        .map(|field| match other.get(field.as_ref()) {
            Some(i) => Ok(*i),
            None => Err(field.as_ref()),
        })
        .collect()
}

/// Helper function for replacing parts of a given text
/// with a new text and writing the result to an io::Write instance.
/// Requires an iterator over (start, end) positions.
#[inline(always)]
pub fn replace_iter<M, W>(
    text: &[u8],
    replacement: &[u8],
    matches: M,
    out: &mut W,
) -> io::Result<()>
where
    M: Iterator<Item = (usize, usize)>,
    W: io::Write + ?Sized,
{
    replace_iter_custom(text, matches, out, |out, _, _| out.write_all(replacement))
}

/// Like replace_iter, but with custom replacement function,
/// which is given the matched text and all remaining text
/// and allows writing anything to the output stream.
#[inline(always)]
pub fn replace_iter_custom<R, M, W>(
    text: &[u8],
    matches: M,
    out: &mut W,
    mut write_replacement: R,
) -> io::Result<()>
where
    R: FnMut(&mut W, &[u8], &[u8]) -> io::Result<()>,
    M: Iterator<Item = (usize, usize)>,
    W: io::Write + ?Sized,
{
    let mut last_end = 0;
    for (start, end) in matches {
        out.write_all(&text[last_end..start])?;
        write_replacement(out, &text[start..end], &text[end..])?;
        last_end = end;
    }
    out.write_all(&text[last_end..])?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use std::io::Write;

    #[test]
    fn replace_iter() {
        let pos = &[(1, 2), (4, 6), (7, 8)];
        let text = b"012345678";
        let replaced = b"0x23x6x8";

        let mut out = vec![];
        super::replace_iter_custom(text, pos.iter().cloned(), &mut out, |out, _, _| {
            out.write_all(b"x")
        })
        .unwrap();
        assert_eq!(&out, replaced);

        // let mut out = vec![];
        // unsafe { super::replace_iter_unchecked(text, b"x", &mut out, pos.iter().cloned()) };
        // assert_eq!(&out, replaced)
    }
}
