use memchr::memchr;

/// Parses a key-value string given any separators
/// "key=value", but not "ke y=value" or " key=value"
/// I wasn't able to write a nom parser faster than this function.
#[inline]
pub fn parse(input: &[u8], delim: u8, value_delim: u8) -> Option<(&[u8], usize, usize)> {
    if let Some(p1) = memchr(value_delim, input) {
        let (k, v) = input.split_at(p1);
        if p1 == 0 || memchr(delim, k).is_some() {
            return None;
        }
        let (vdelim, end) = if let Some(end) = memchr(delim, v) {
            (p1, p1 + end)
        } else {
            (p1, input.len())
        };
        Some((k, vdelim + 1, end))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_kv(input: &[u8]) -> Option<(&[u8], usize, usize)> {
        parse(input, b' ', b'=')
    }

    #[test]
    fn test_key_value() {
        assert_eq!(std_kv(&b"k=v "[..]), Some((&b"k"[..], 2, 3)));
        assert_eq!(std_kv(&b"k=v"[..]), Some((&b"k"[..], 2, 3)));
        assert_eq!(std_kv(&b" k=v "[..]), None);
        assert_eq!(std_kv(&b"key=value "[..]), Some((&b"key"[..], 4, 9)));
        assert_eq!(std_kv(&b"ke y=value "[..]), None);
        assert_eq!(std_kv(&b"=v"[..]), None);
    }
}
