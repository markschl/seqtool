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

/// Helper function for replacing parts of a given text,
/// and writing the result to an io::Write instance.
/// Needs an iterator over (start, end) positions.
/// A custom function for writing has to be supplied,
/// which is also given the matched text and all remaining text.
#[inline(always)]
pub fn replace_iter<R, M, W>(
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

// pub unsafe fn replace_iter_unchecked<M>(text: &[u8], replacement: &[u8], out: &mut Vec<u8>, matches: M)
// where
//     M: Iterator<Item = (usize, usize)>,
// {
//     let mut last_end = 0;
//     for (start, end) in matches {
//         out.extend_from_slice(unsafe { get_unchecked(text, last_end, start) });
//         out.extend_from_slice(replacement);
//         last_end = end;
//     }
//     out.extend_from_slice(unsafe { get_unchecked(text, last_end, text.len()) });
// }
//
// #[inline]
// unsafe fn get_unchecked(text: &[u8], start: usize, end: usize) -> &[u8] {
//     let ptr = text.as_ptr().offset(start as isize);
//     std::slice::from_raw_parts(ptr, end - start)
// }

pub fn text_to_int(text: &[u8]) -> Result<i64, String> {
    atoi::atoi(text).ok_or_else(|| {
        format!(
            "Could not convert '{}' to decimal number.",
            String::from_utf8_lossy(text)
        )
    })
}

pub fn text_to_float(text: &[u8]) -> Result<f64, String> {
    // TODO: any more efficient way?
    std::str::from_utf8(text)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            format!(
                "Could not convert '{}' to integer.",
                String::from_utf8_lossy(text)
            )
        })
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
        super::replace_iter(text, pos.iter().cloned(), &mut out, |out, _, _| {
            out.write_all(b"x")
        })
        .unwrap();
        assert_eq!(&out, replaced);

        // let mut out = vec![];
        // unsafe { super::replace_iter_unchecked(text, b"x", &mut out, pos.iter().cloned()) };
        // assert_eq!(&out, replaced)
    }
}
