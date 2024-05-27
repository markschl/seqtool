use std::io;

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
