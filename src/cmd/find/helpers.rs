use std::fmt::Display;

use itertools::Itertools;

use crate::cmd::shared::seqtype::{guess_seqtype, SeqType};
use crate::error::CliResult;
use crate::io::RecordAttr;

use super::matcher::{BytesRegexMatcher, ExactMatcher, Matcher, MyersMatcher, RegexMatcher};
use super::MatchOpts;

static AMBIG_DNA: &[(u8, &[u8])] = &[
    (b'M', b"AC"),
    (b'R', b"AG"),
    (b'W', b"AT"),
    (b'S', b"CG"),
    (b'Y', b"CT"),
    (b'K', b"GT"),
    (b'V', b"ACGMRS"),
    (b'H', b"ACTMWY"),
    (b'D', b"AGTRWK"),
    (b'B', b"CGTSYK"),
    (b'N', b"ACGTMRWSYKVHDB"),
];

static AMBIG_PROTEIN: &[(u8, &[u8])] = &[(b'X', b"CDEFGHIKLMNOPQRSTUVWY")];

lazy_static! {
    static ref _AMBIG_RNA: Vec<(u8, Vec<u8>)> = AMBIG_DNA
        .iter()
        .map(|(b, eq)| {
            let eq = eq
                .iter()
                .map(|&b| if b == b'T' { b'U' } else { b })
                .collect();
            (*b, eq)
        })
        .collect();
    static ref AMBIG_RNA: Vec<(u8, &'static [u8])> = _AMBIG_RNA
        .iter()
        .map(|(b, eq)| (*b, eq.as_slice()))
        .collect();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    Exact,
    Regex,
    Myers,
}

pub fn algorithm_from_name(s: &str) -> Result<Option<Algorithm>, String> {
    match &*s.to_ascii_lowercase() {
        "exact" => Ok(Some(Algorithm::Exact)),
        "regex" => Ok(Some(Algorithm::Regex)),
        "myers" => Ok(Some(Algorithm::Myers)),
        "auto" => Ok(None),
        _ => Err(format!("Unknown search algorithm: {}", s)),
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
        && dist > 0
    {
        eprintln!("Warning: distance option ignored with exact/regex matching.");
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
    attr: RecordAttr,
    ambig: bool,
    o: &MatchOpts,
) -> CliResult<Box<dyn Matcher + Send + 'a>> {
    if algorithm != Algorithm::Regex && o.has_groups {
        return fail!("Match groups > 0 can only be used with regular expression searches.");
    }
    Ok(match algorithm {
        Algorithm::Exact => Box::new(ExactMatcher::new(pattern.as_bytes())),
        Algorithm::Regex => {
            if attr == RecordAttr::Seq {
                Box::new(BytesRegexMatcher::new(pattern, o.has_groups)?)
            } else {
                Box::new(RegexMatcher::new(pattern, o.has_groups)?)
            }
        }
        Algorithm::Myers => {
            let ambig_map = if ambig {
                match o.seqtype {
                    SeqType::Dna => Some(AMBIG_DNA),
                    SeqType::Rna => Some(&AMBIG_RNA as &[(u8, &[u8])]),
                    SeqType::Protein => Some(AMBIG_PROTEIN),
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
        return fail!(
            "Pattern file is empty: {}. Patterns should be in FASTA format.",
            path
        );
    }
    Ok(out)
}
