//use std::ops::Deref;
use std::str::FromStr;
use std::convert::AsRef;
use std::collections::HashMap;

use error::CliResult;
use lib::inner_result::MapRes;

pub fn version() -> String {
    let (maj, min, pat) = (
        option_env!("CARGO_PKG_VERSION_MAJOR"),
        option_env!("CARGO_PKG_VERSION_MINOR"),
        option_env!("CARGO_PKG_VERSION_PATCH"),
    );
    match (maj, min, pat) {
        (Some(maj), Some(min), Some(pat)) => format!("{}.{}.{}", maj, min, pat),
        _ => "".to_string(),
    }
}

pub fn parse_delimiter(delim: &str) -> CliResult<u8> {
    match delim {
        r"\t" => Ok(b'\t'),
        _ => {
            if delim.len() != 1 {
                Err(format!(
                    "Invalid delimiter: '{}'. Only 1-character delimiters are possible.",
                    delim
                ))?
            }
            Ok(delim.as_bytes()[0])
        }
    }
}

pub fn match_fields<'a, S1, S2>(fields: &'a [S1], other: &'a [S2]) -> Result<Vec<usize>, &'a str>
where
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let other: HashMap<_, _> = other
        .into_iter()
        .enumerate()
        .map(|(i, f)| (f.as_ref(), i))
        .collect();

    fields
        .into_iter()
        .map(|field| match other.get(field.as_ref()) {
            Some(i) => Ok(*i),
            None => Err(field.as_ref()),
        })
        .collect()
}

pub fn parse_range_str(range: &str) -> CliResult<(Option<&str>, Option<&str>)> {
    let rng: Vec<&str> = range
        .splitn(2, "..")
        .map(|r| r.trim())
        .collect();

    if rng.len() != 2 {
        return fail!(format!(
            "Invalid range: '{}'. Possible notations: 'start..end' or 'start..' or '..end', or '..'",
            range
        ));
    }

    let start = if rng[0].is_empty() {
        None
    } else {
        Some(rng[0])
    };
    let end = if rng[1].is_empty() {
        None
    } else {
        Some(rng[1])
    };
    Ok((start, end))
}

pub fn parse_range<T: FromStr>(range: &str) -> CliResult<(Option<T>, Option<T>)> {
    let (start, end) = parse_range_str(range)?;
    Ok((
        start.map_res(|s| {
            s.trim()
                .parse::<T>()
                .map_err(|_| format!("Invalid range start: '{}'.", s))
        })?,
        end.map_res(|e| {
            e.trim()
                .parse::<T>()
                .map_err(|_| format!("Invalid range end: '{}'.", e))
        })?,
    ))
}

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
    out.extend_from_slice(&text[last_end..text.len()]);
}
