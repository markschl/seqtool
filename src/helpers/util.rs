//use std::ops::Deref;
use std::collections::HashMap;
use std::convert::AsRef;

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

#[inline]
pub fn replace_iter<M>(text: &[u8], replacement: &[u8], out: &mut Vec<u8>, matches: M)
where
    M: Iterator<Item = (usize, usize)>,
{
    let mut last_end = 0;
    for (start, end) in matches {
        out.extend_from_slice(&text[last_end..start]);
        out.extend_from_slice(replacement);
        last_end = end;
    }
    out.extend_from_slice(&text[last_end..]);
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

#[cfg(test)]
mod tests {
    #[test]
    fn replace_iter() {
        let pos = &[(1, 2), (4, 6), (7, 8)];
        let text = b"012345678";
        let replaced = b"0x23x6x8";

        let mut out = vec![];
        super::replace_iter(text, b"x", &mut out, pos.iter().cloned());
        assert_eq!(&out, replaced);

        // let mut out = vec![];
        // unsafe { super::replace_iter_unchecked(text, b"x", &mut out, pos.iter().cloned()) };
        // assert_eq!(&out, replaced)
    }
}
