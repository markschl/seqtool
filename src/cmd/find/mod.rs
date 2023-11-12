use std::collections::HashMap;
use std::fmt::Display;
use std::str;

use itertools::Itertools;

use crate::config;
use crate::error::{CliError, CliResult};
use crate::io::output::writer::Writer;
use crate::io::{RecordEditor, SeqAttr};
use crate::helpers::rng::Range;
use crate::helpers::seqtype::{guess_seqtype, SeqType};
use crate::helpers::util::{parse_range, replace_iter};
use crate::opt;
use crate::var::{varstring, VarHelp, VarProvider};

use self::matcher::*;
use self::matches::*;
use self::vars::*;

mod matcher;
mod matches;
mod vars;

static USAGE: &str = concat!(
    "
Fast searching for one or more patterns in sequences or ids/descriptions,
with optional multithreading.

Usage:
  st find [options] [-a <attr>...][-l <list>...] <pattern> [<input>...]
  st find (-h | --help)
  st find --help-vars

Search Options:
    <pattern>           Pattern string or 'file:<patterns.fasta>'
    -r, --regex         Treat the pattern(s) as regular expressions.
    -d, --dist <dist>   Fuzzy string matching with maximum edit distance
                        of <dist> [default: 0]
    --in-order          Report hits in the order of their occurrence instead
                        of sorting by distance (with -d > 0)
    --seqtype <type>    Sequence type {dna/rna/protein/other}
    -t, --threads <N>   Number of threads to use [default: 1]
    --ambig <yn>        Override choice of whether DNA ambiguity codes (IUPAC)
                        are recognized or not {yes/no}.
    --algo <algorithm>  Override decision of algorithm for testing
                        (regex/exact/myers/auto) [default: auto]

Search range:
    --rng <range>       Search within the given range ('start..end', 'start..'
                        or '..end'). Using variables is not possible.
    --max-shift-l <n>   Consider only matches with a maximum distance of <n> from
                        the search start (eventually > 1 if using --rng)
    --max-shift-r <n>   Consider only matches with a maximum distance from the
                        end of the search range

Attributes:
    -i, --id            Search / replace in IDs instead of sequences
    --desc              Search / replace in descriptions

Actions:
    -f, --filter        Keep only matching sequences
    -e, --exclude       Exclude sequences that matched
    --dropped <file>    Output file for sequences that were removed by filtering.
                        The extension is autorecognized if possible, fallback
                        is the input format.
    --rep <with>        Replace by a composable string
",
    common_opts!()
);

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

use self::Algorithm::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Algorithm {
    Exact,
    Regex,
    Myers,
}

impl Algorithm {
    fn from_str(s: &str) -> Option<Algorithm> {
        Some(match &*s.to_ascii_lowercase() {
            "exact" => Exact,
            "regex" => Regex,
            "myers" => Myers,
            _ => return None,
        })
    }
}

struct MatchOpts {
    has_groups: bool,
    bounds_needed: bool,
    sorted: bool,
    max_dist: usize,
    seqtype: SeqType,
}

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args_with_help(&args, &FindVarHelp)?;

    let max_dist: usize = args.value("--dist")?;
    let sorted = !args.get_bool("--in-order");
    let regex = args.get_bool("--regex");
    let ambig = args.yes_no("--ambig")?;
    let verbose = args.get_bool("--verbose");

    let attr = if args.get_bool("--id") {
        SeqAttr::Id
    } else if args.get_bool("--desc") {
        SeqAttr::Desc
    } else {
        SeqAttr::Seq
    };

    let flt = args.get_bool("--filter");
    let filter = if flt {
        Some(true)
    } else if args.get_bool("--exclude") {
        if flt {
            return fail!("-f/--filter and -e/--exclude cannot both be specified");
        }
        Some(false)
    } else {
        None
    };

    let num_threads = args.thread_num()?;

    let pattern = args.get_str("<pattern>");
    let patterns = if !pattern.starts_with("file:") {
        vec![("<pattern>".to_string(), pattern.to_string())]
    } else {
        read_pattern_file(&pattern[5..])?
    };

    let typehint = args.opt_str("--seqtype").map(|s| s.to_ascii_lowercase());

    //let replace_num = args.get_str("--match-num");
    let replacement = args.opt_str("--rep");

    let range = if let Some(r) = args.opt_str("--rng") {
        let (start, end) = parse_range(r)?;
        Some((start.unwrap_or(1), end.unwrap_or(-1)))
    } else {
        None
    };

    let max_shift = if let Some(n) = args.opt_str("--max-shift-l") {
        Some(Shift::Start(n.parse().map_err(|_| {
            format!("Invalid max. left shift value: {}", n)
        })?))
    } else if let Some(n) = args.opt_str("--max-shift-r") {
        Some(Shift::End(n.parse().map_err(|_| {
            format!("Invalid max. right shift value: {}", n)
        })?))
    } else {
        None
    };

    let dropped_file = args.opt_str("--dropped").map(|s| s.to_string());

    // override algorithm for testing
    let algo_override = Algorithm::from_str(args.get_str("--algo"));

    ///// option parsing end

    // determine sequence type for each pattern
    let typehint = typehint.as_deref();
    let (seqtype, algorithms) = analyse_patterns(
        &patterns,
        algo_override,
        typehint,
        ambig,
        regex,
        max_dist,
        verbose,
    )?;

    cfg.with_vars(|vars| {

        vars.add_module(FindVars::new(patterns.len()));

        // run
        cfg.writer_with(vars, |writer, mut vars| {

            // make sure all hits for group 0 are collected (group 0 is always searched)
            // API is somehow awkward
            let replacement = if let Some(r) = replacement {
                vars.last_module_as::<FindVars, _>(|v, _| {
                    v.register_all(0);
                    Ok(())
                })?;
                let s = vars.build(|b| {
                    varstring::VarString::var_or_composed(r, b)
                })?;
                Some(s)
            } else {
                None
            };

            let (match_cfg, opts) = vars.last_module_as::<FindVars, _>(|match_vars, _| {

                if filter.is_none() && !match_vars.has_vars() && replacement.is_none() {
                    return fail!(
                        "Find command does nothing. Use -f/-e for filtering, --repl for replacing or \
                            -a for writing attributes."
                    );
                }

                let bounds_needed =
                    match_vars.bounds_needed().0 || match_vars.bounds_needed().1 || max_shift.is_some();

                report!(
                    verbose,
                    "Sort by distance: {:?}. Find full position: {:?}",
                    sorted,
                    bounds_needed
                );

                let match_cfg = match_vars.config().clone();

                let opts = MatchOpts {
                    has_groups: match_cfg.has_groups(),
                    bounds_needed,
                    sorted,
                    max_dist,
                    seqtype,
                };

                Ok((match_cfg, opts))
            })?;

            let mut replacement_text = vec![];
            let (pattern_names, patterns): (Vec<_>, Vec<_>) = patterns.into_iter().unzip();

            let mut dropped_file = if let Some(f) = dropped_file.as_ref() {
                Some(cfg.other_writer(f, Some(&mut vars))?)
            } else {
                None
            };

            cfg.parallel_init_var(
                vars,
                num_threads,
                || {
                    // initialize matchers (one per record set)
                    algorithms
                        .iter()
                        .zip(&patterns)
                        .map(|(&(algo, is_ambig), patt)| get_matcher(patt, algo, is_ambig, &opts))
                        .collect::<CliResult<Vec<_>>>()
                },
                || {
                    // initialize per-sequence record data
                    let editor = Box::<RecordEditor>::default();
                    let matches = Box::new(Matches::new(
                        &pattern_names,
                        match_cfg.clone(),
                        range,
                        max_shift.clone(),
                    ));
                    (editor, matches)
                },
                |record, &mut (ref mut editor, ref mut matches), ref mut matchers| {
                    // searching in worker threads
                    let text = editor.get(attr, &record, false);
                    matches.find(text, matchers);
                    Ok(())
                },
                |record, &mut (ref mut editor, ref matches), vars| {
                    vars.last_module_as::<FindVars, _>(|match_vars, symbols| {
                        // records returned to main thread
                        if let Some(rep) = replacement.as_ref() {
                            editor.edit_with_val(attr, &record, true, |text, out| {
                                match_vars.set_with(record, matches, symbols, text)?;
                                replacement_text.clear();
                                rep.compose(&mut replacement_text, symbols, record);

                                let pos = matches
                                    .matches_iter(0, 0)
                                    .flatten()
                                    .map(|m| (m.start, m.end));
                                replace_iter(text, &replacement_text, out, pos);

                                Ok::<(), CliError>(())
                            })?;
                        } else {
                            let text = editor.get(attr, &record, true);
                            match_vars.set_with(record, matches, symbols, text)?;
                        }
                        Ok(())
                    })?;

                    // keep / exclude
                    if let Some(keep) = filter {
                        if matches.has_matches() ^ keep {
                            if let Some(ref mut f) = dropped_file {
                                f.write(&record, vars)?;
                            }
                            return Ok(true);
                        }
                    }

                    // write non-excluded to output
                    writer.write(&editor.rec(&record), vars)?;
                    Ok(true)
                },
            )?;
            Ok(())
        })?;
        Ok(())
    })
}

fn analyse_patterns<S>(
    patterns: &[(S, S)],
    algo_override: Option<Algorithm>,
    typehint: Option<&str>,
    ambig_override: Option<bool>,
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
                      typehint.unwrap_or("<nothing>")
                    )
                })?;
            // no discrimination here
            let mut is_ambig = is_n || is_ambig;

            if is_ambig {
                ambig_seqs.push(name.as_ref());
            }

            if seqtype == SeqType::Other && ambig_override.unwrap_or(false) {
                eprintln!(
                  "Warning: Ambiguous matching was activated, but the sequence type of the pattern \
                  '{}' does not seem to be DNA/RNA/protein.",
                  name
              );
            }

            is_ambig = ambig_override.unwrap_or(is_ambig);

            // decide which algorithm should be used
            let mut algorithm = if regex {
                Regex
            } else if dist > 0 || is_ambig {
                Myers
            } else {
                Exact
            };

            // override with user choice
            if let Some(a) = algo_override {
                algorithm = a;
                if a != Myers && is_ambig {
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

    if let Some(a) = ambig_override {
        if ambig_seqs.is_empty() {
            if a {
                eprintln!(
                    "Warning: Ambiguous matching was activated (--ambig yes), but there is no \
                     pattern with ambiguous characters"
                );
            }
        } else if !a {
            eprintln!(
                "Warning: Ambiguous matching is deactivated (--ambig no), but there are patterns \
                 with ambiguous characters ({})",
                ambig_seqs.join(", ")
            );
        }
    }

    if out.iter().any(|&(a, _)| a == Regex || a == Exact) {
        if dist > 0 {
            eprintln!("Warning: distance option ignored.");
        }
        if ambig_override.is_some() {
            eprintln!("Warning: '--ambig' ignored.");
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

fn get_matcher<'a>(
    pattern: &str,
    algorithm: Algorithm,
    ambig: bool,
    o: &MatchOpts,
) -> CliResult<Box<dyn Matcher + Send + 'a>> {
    if algorithm != Regex && o.has_groups {
        return fail!("Match groups > 0 can only be used with regular expression searches.");
    }
    Ok(match algorithm {
        Exact => Box::new(ExactMatcher::new(pattern.as_bytes())),
        Regex =>
        // TODO: string regexes for ID/desc
        {
            Box::new(BytesRegexMatcher::new(pattern, o.has_groups)?)
        }
        Myers => {
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

fn read_pattern_file(path: &str) -> CliResult<Vec<(String, String)>> {
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
