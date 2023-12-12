use std::{collections::HashMap, fmt::Display};

use itertools::Itertools;

use crate::{
    error::CliResult,
    helpers::seqtype::{guess_seqtype, SeqType},
};

use super::{
    matcher::{BytesRegexMatcher, ExactMatcher, Matcher, MyersMatcher},
    MatchOpts,
};

lazy_static! {
    static ref AMBIG_DNA: HashMap<u8, Vec<u8>> = hashmap! {
        b'M' => b"AC".to_vec(),
        b'R' => b"AG".to_vec(),
        b'W' => b"AT".to_vec(),
        b'S' => b"CG".to_vec(),
        b'Y' => b"CT".to_vec(),
        b'K' => b"GT".to_vec(),
        b'V' => b"ACGMRS".to_vec(),
        b'H' => b"ACTMWY".to_vec(),
        b'D' => b"AGTRWK".to_vec(),
        b'B' => b"CGTSYK".to_vec(),
        b'N' => b"ACGTMRWSYKVHDB".to_vec(),
    };
    static ref AMBIG_RNA: HashMap<u8, Vec<u8>> = AMBIG_DNA
        .iter()
        .map(|(&b, eq)| {
            let eq = eq
                .iter()
                .map(|&b| if b == b'T' { b'U' } else { b })
                .collect();
            (b, eq)
        })
        .collect();
    static ref AMBIG_PROTEIN: HashMap<u8, Vec<u8>> = hashmap! {
        b'X' => b"CDEFGHIKLMNOPQRSTUVWY".to_vec(),
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    Exact,
    Regex,
    Myers,
}

impl Algorithm {
    pub fn from_str(s: &str) -> Option<Algorithm> {
        Some(match &*s.to_ascii_lowercase() {
            "exact" => Algorithm::Exact,
            "regex" => Algorithm::Regex,
            "myers" => Algorithm::Myers,
            _ => return None,
        })
    }
}

pub(crate) fn analyse_patterns<S>(
    patterns: &[(S, S)],
    algo_override: Option<Algorithm>,
    typehint: Option<SeqType>,
    no_ambig: bool,
    regex: bool,
    dist: usize,
    verbose: bool,
) -> CliResult<(SeqType, Vec<(Algorithm, bool)>)>
where
    S: AsRef<str> + Display,
{
    use std::collections::HashSet;
    let mut ambig_seqs = vec![];

    let (unique_seqtypes, out): (HashSet<SeqType>, Vec<(Algorithm, bool)>) = patterns
        .iter()
        .map(|(name, pattern)| {
            let (seqtype, is_n, is_ambig) = guess_seqtype(pattern.as_ref().as_bytes(), typehint)
                .ok_or_else(|| {
                    format!(
                      "{} was specified as sequence type, but sequence recognition suggests another type.",
                      typehint.map(|t| t.to_string()).unwrap_or("<nothing>".to_string())
                    )
                })?;
            // no discrimination here
            let mut is_ambig = is_n || is_ambig;
            if is_ambig {
                ambig_seqs.push(name.as_ref());
            }
            // override if no_ambig was set
            if no_ambig {
                is_ambig = false;
            }

            // decide which algorithm should be used
            let mut algorithm = if regex {
                Algorithm::Regex
            } else if dist > 0 || is_ambig {
                Algorithm::Myers
            } else {
                Algorithm::Exact
            };

            // override with user choice
            if let Some(a) = algo_override {
                algorithm = a;
                if a != Algorithm::Myers && is_ambig {
                    eprintln!("Warning: --ambig ignored.");
                    is_ambig = false;
                }
            }

            report!(
                verbose,
                "{}: {:?}{}, search algorithm: {:?}{}",
                name,
                seqtype,
                if is_ambig { " with ambiguities" } else { "" },
                algorithm,
                if dist > 0 {
                    format!(", max. distance: {}", dist)
                } else {
                    "".to_string()
                },
            );

            Ok((seqtype, (algorithm, is_ambig)))
        })
        .collect::<CliResult<Vec<_>>>()?
        .into_iter()
        .unzip();

    if no_ambig && !ambig_seqs.is_empty() {
        eprintln!(
            "Warning: Ambiguous matching is deactivated (--no-ambig), but there are patterns \
        with ambiguous characters ({})",
            ambig_seqs.join(", ")
        );
    }

    if out
        .iter()
        .any(|&(a, _)| a == Algorithm::Regex || a == Algorithm::Exact)
    {
        if dist > 0 {
            eprintln!("Warning: distance option ignored with exact/regex matching.");
        }
    }

    if unique_seqtypes.len() > 1 {
        return fail!(format!(
        "Autorecognition of sequence types patterns suggests that there are different types ({}).\
    Please specify the type with --seqtype",
        unique_seqtypes.iter().map(|t| format!("{:?}", t)).join(", ")
      ));
    }

    let t = unique_seqtypes.into_iter().next().unwrap();
    Ok((t, out))
}

pub(crate) fn get_matcher<'a>(
    pattern: &str,
    algorithm: Algorithm,
    ambig: bool,
    o: &MatchOpts,
) -> CliResult<Box<dyn Matcher + Send + 'a>> {
    if algorithm != Algorithm::Regex && o.has_groups {
        return fail!("Match groups > 0 can only be used with regular expression searches.");
    }
    Ok(match algorithm {
        Algorithm::Exact => Box::new(ExactMatcher::new(pattern.as_bytes())),
        // TODO: string regexes for ID/desc
        Algorithm::Regex => Box::new(BytesRegexMatcher::new(pattern, o.has_groups)?),
        Algorithm::Myers => {
            let ambig_map = if ambig {
                match o.seqtype {
                    SeqType::Dna => Some(&AMBIG_DNA as &HashMap<_, _>),
                    SeqType::Rna => Some(&AMBIG_RNA as &HashMap<_, _>),
                    SeqType::Protein => Some(&AMBIG_PROTEIN as &HashMap<_, _>),
                    SeqType::Other => None,
                }
            } else {
                None
            };
            Box::new(MyersMatcher::new(
                pattern.as_bytes(),
                o.max_dist,
                o.bounds_needed,
                o.sorted,
                ambig_map,
            )?)
        }
    })
}

pub(crate) fn read_pattern_file(path: &str) -> CliResult<Vec<(String, String)>> {
    use seq_io::fasta::*;
    let mut reader = Reader::from_path(path)?;
    let mut out = vec![];
    while let Some(r) = reader.next() {
        let r = r?;
        out.push((r.id()?.to_string(), String::from_utf8(r.seq().to_owned())?));
    }
    if out.is_empty() {
        return fail!("Pattern file is empty.");
    }
    Ok(out)
}
