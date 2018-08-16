//! Small function that parses memory sizes, accepting
//! different units (K, M, G, T)

pub fn parse_bytesize(size: &str) -> Result<f64, String> {
    let size = size.trim();
    if size.is_empty() {
        return Err("Empty size string.".to_string());
    }
    let number = size.parse::<f64>();

    match number {
        Ok(n) => Ok(n),
        Err(_) => {
            let (unit_size, unit) = size.split_at(size.len() - 1);
            let suffixes = [b'B', b'K', b'M', b'G', b'T']; //, "P", "E"]
            let unit_byte = unit.to_ascii_uppercase().as_bytes()[0];
            if let Some(pow) = suffixes.iter().position(|s| *s == unit_byte) {
                if let Ok(s) = unit_size.trim().parse::<f64>() {
                    Ok(s * (1024_f64).powi(pow as i32))
                } else {
                    Err(format!("Invalid size string: '{}'", size))
                }
            } else {
                Err(format!("Unknown size unit: '{}'", unit))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytesize() {
        assert_relative_eq!(parse_bytesize("1.").unwrap(), 1.);
        assert_relative_eq!(parse_bytesize(" 1 B").unwrap(), 1.);
        assert_relative_eq!(parse_bytesize(" 100K ").unwrap(), 100. * 1024.);
        assert_relative_eq!(parse_bytesize("2.3M").unwrap(), 2.3 * 1024. * 1024.);
        assert_relative_eq!(parse_bytesize("2.3M").unwrap(), 2.3 * 1024. * 1024.);
        assert_relative_eq!(parse_bytesize("9 g").unwrap(), 9. * 1024. * 1024. * 1024.);
        assert_relative_eq!(
            parse_bytesize("1T").unwrap(),
            1. * 1024. * 1024. * 1024. * 1024.
        );
        assert!(parse_bytesize("x").is_err());
        assert!(parse_bytesize("1x").is_err());
    }
}
