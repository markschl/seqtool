use std::{collections::HashSet, fmt::Display};

use itertools::Itertools;

use crate::{
    helpers::seqtype::{guess_seqtype_or_fail, SeqType, SeqTypeInfo},
    CliResult,
};

use super::opts::{Algorithm, DistanceThreshold};

pub(crate) fn analyse_patterns<S>(
    patterns: &[(Option<S>, S)],
    algo_override: Option<Algorithm>,
    typehint: Option<SeqType>,
    no_ambig: bool,
    regex: bool,
    max_dist: Option<DistanceThreshold>,
    quiet: bool,
) -> CliResult<(SeqType, Vec<(Algorithm, bool)>)>
where
    S: AsRef<str> + Display,
{
    let mut ambig_seqs = vec![];

    let (unique_seqtypes, out): (HashSet<SeqType>, Vec<(Algorithm, bool)>) = patterns
        .iter()
        .map(|(name, pattern)| {
            let info = if regex {
                SeqTypeInfo::new(SeqType::Other, false, false)
            } else {
                guess_seqtype_or_fail(pattern.as_ref().as_bytes(), typehint, true).map_err(|e| {
                    format!(
                        "Error in search pattern{}: {}",
                        name.as_ref()
                            .map(|n| format!(" '{}'", n.as_ref()))
                            .unwrap_or_default(),
                        e
                    )
                })?
            };
            // no discrimination here
            let mut has_ambig = info.has_wildcard || info.has_ambiguities;
            if has_ambig {
                ambig_seqs.push(name.as_ref());
            }
            // override if no_ambig was set
            if no_ambig {
                has_ambig = false;
            }

            // decide which algorithm should be used
            let mut algorithm = if regex {
                Algorithm::Regex
            } else if max_dist.is_some() || has_ambig {
                Algorithm::Myers
            } else {
                Algorithm::Exact
            };

            // override with user choice
            if let Some(a) = algo_override {
                algorithm = a;
                if a != Algorithm::Myers && has_ambig {
                    eprintln!("Warning: `--ambig` ignored with search algorithm '{}'.", a);
                    has_ambig = false;
                }
            }

            if typehint.is_none() && algorithm != Algorithm::Regex && !quiet {
                // unless 'regex' was specified, we must know the correct sequence type,
                // or there could be unexpected behaviour
                eprint!("Note: the sequence type of the pattern ",);
                if let Some(n) = name {
                    eprint!("'{}' ", n);
                }
                eprint!("was determined as '{}'", info.seqtype);
                if has_ambig {
                    eprint!(" (with ambiguous letters)");
                }
                eprintln!(
                    ". If incorrect, please provide the correct type with `--seqtype`. \
                    Use `-q/--quiet` to suppress this message."
                );
            }

            Ok((info.seqtype, (algorithm, has_ambig)))
        })
        .collect::<CliResult<Vec<_>>>()?
        .into_iter()
        .unzip();

    if no_ambig && !ambig_seqs.is_empty() && !quiet {
        eprintln!(
            "Warning: Ambiguous matching is deactivated (--no-ambig), but there are patterns \
            with ambiguous letters ({}). Use `-q/--quiet` to suppress this message.",
            ambig_seqs.iter().map(|s| s.unwrap()).join(", ") // unwrap: >1 patterns means they are all named
        );
    }

    if out
        .iter()
        .any(|&(a, _)| a == Algorithm::Regex || a == Algorithm::Exact)
        && max_dist.is_some()
        && !quiet
    {
        eprintln!(
            "Warning: `-D/--max-diffs` option ignored with exact/regex matching. \
            Use `-q/--quiet` to suppress this message."
        );
    }

    if unique_seqtypes.len() > 1 {
        return fail!(format!(
            "Autorecognition of pattern sequence types suggests that there are \
            several different types ({}). Please specify the correct type with --seqtype",
            unique_seqtypes
                .iter()
                .map(|t| format!("{:?}", t))
                .join(", ")
        ));
    }

    let t = unique_seqtypes.into_iter().next().unwrap();
    Ok((t, out))
}
