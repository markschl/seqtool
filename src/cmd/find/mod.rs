use std::str;
use std::fmt::Display;
use std::ascii::AsciiExt;

use itertools::Itertools;

use error::CliResult;
use opt;
use cfg;
use var::{varstring, VarHelp, VarProvider};
use io::{SeqAttr, RecordEditor};
use io::output::writer::Writer;
use lib::util::{parse_range, replace_iter};
use lib::subst_matrix::AsymmIdentityDnaMatrix;
use lib::rng::Range;
use lib::seqtype::{guess_seqtype, SeqType};

use bio::pattern_matching::ukkonen::unit_cost;

use self::matcher::*;
use self::matches::*;
use self::vars::*;

mod matcher;
mod matches;
mod vars;

static USAGE: &'static str = concat!("
Searches for one or more patterns in sequences or ids / descriptions,
optional multithreading.

Usage:
  seqtool find [options] [-a <attr>...] <pattern> [<input>...]
  seqtool find (-h | --help)
  seqtool find --help-vars

Search Options:
    <pattern>           Pattern string or 'file:<patterns.fasta>'
    -r, --regex         Treat the pattern(s) as regular expressions.
    -d, --dist <dist>   Approximative string matching with maximum edit distance
                        of <dist> [default: 0]
    --in-order          Report hits in the order of their occurrence instead
                        of sorting by distance (might be slower because -g yes)
    -g, --group <yes>   Group hits by starting position (keep only the best one.
                        (slower) {yes/no}. default: 'no', unless --in-order is used.
    -a, --ambig <yes>   Override choice of whether DNA ambiguity codes (IUPAC)
                        are recognized or not {yes/no}.
    --seqtype <type>    Sequence type {dna/protein/other}
    -t, --threads <N>   Number of threads to use [default: 1]
    --algo <algorithm>  Override decision of algorithm for testing
                        (regex/exact/ukkonen/myers/auto) [default: auto]

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

use self::Algorithm::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Algorithm {
    Exact,
    Regex,
    Ukkonen,
    Myers,
}

impl Algorithm {
    fn from_str(s: &str) -> Option<Algorithm> {
        Some(match &*s.to_ascii_lowercase() {
            "exact" => Exact,
            "regex" => Regex,
            "ukkonen" => Ukkonen,
            "myers" => Myers,
            _ => return None,
        })
    }
}

struct MatchOpts {
    has_groups: bool,
    needs_alignment: bool,
    sorted: bool,
    group_pos: bool,
    max_dist: u16,
    seqtype: SeqType,
}

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args_with_help(&args, &FindVarHelp)?;

    let dist: u16 = args.value("--dist")?;
    let sorted = !args.get_bool("--in-order");
    let regex = args.get_bool("--regex");
    let ambig = args.yes_no("--ambig")?;
    let group_pos = args.yes_no("--group")?.unwrap_or_else(|| !sorted);
    let verbose = args.get_bool("--verbose");

    let attr = if args.get_bool("--id") {
        SeqAttr::Id
    } else if args.get_bool("--desc") {
        SeqAttr::Desc
    } else {
        SeqAttr::Seq
    };

    let filter = if args.get_bool("--filter") {
        Some(true)
    } else if args.get_bool("--exclude") {
        Some(false)
    } else {
        None
    };

    let num_threads = args.thread_num()?;

    let pattern = args.get_str("<pattern>");
    let patterns = if !pattern.starts_with("file:") {
        vec![("".to_string(), pattern.to_string())]
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
    let typehint = typehint.as_ref().map(|s| s.as_str());
    let (seqtype, algorithms) = analyse_patterns(
        &patterns,
        algo_override,
        typehint,
        ambig,
        regex,
        dist,
        verbose,
    )?;

    // run
    cfg.writer_with(
        |_| Ok(FindVars::new()),
        |writer, mut vars, mut match_vars| {
            let replacement = if let Some(r) = replacement {
                // make sure all hits for group 0 are collected (group 0 is always searched)
                // API is somehow awkward
                match_vars.register_all(0);
                let s = vars.build_with(Some(&mut match_vars), |b| {
                    varstring::VarString::var_or_composed(r, b)
                })?;
                Some(s)
            } else {
                None
            };

            if filter.is_none() && !match_vars.has_vars() && replacement.is_none() {
                return fail!(
                    "Match command does nothing. Use -f/-e for filtering, --repl for replacing or \
                     -p for writing attributes."
                );
            }

            let needs_alignment =
                match_vars.bounds_needed().0 || match_vars.bounds_needed().1 || max_shift.is_some();

            report!(
              verbose,
              "Sorting by best hit: {:?}, grouping hits by position: {:?}, doing alignments: {:?}",
              sorted, group_pos, needs_alignment
            );

            let opts = MatchOpts {
                has_groups: match_vars.positions().has_groups(),
                needs_alignment: needs_alignment,
                sorted: sorted,
                group_pos: group_pos,
                max_dist: dist,
                seqtype: seqtype,
            };

            let mut replacement_text = vec![];
            let (pattern_names, patterns): (Vec<_>, Vec<_>) = patterns.into_iter().unzip();

            let mut dropped_file = if let Some(f) = dropped_file.as_ref() {
                Some(cfg.other_writer(f, None, None)?)
            } else {
                None
            };

            let pos = match_vars.positions().clone();

            cfg.var_parallel_init(
                &mut vars,
                num_threads,
                || {
                    algorithms
                        .iter()
                        .zip(&patterns)
                        .map(|(&(algo, is_ambig), patt)| get_matcher(patt, algo, is_ambig, &opts))
                        .collect::<CliResult<Vec<_>>>()
                },
                || {
                    let editor = Box::new(RecordEditor::default());
                    let matches = Box::new(Matches::new(
                        &pattern_names,
                        pos.clone(),
                        range,
                        max_shift.clone(),
                    ));
                    (editor, matches)
                },
                |record, &mut (ref mut editor, ref mut matches), ref mut matchers| {
                    let text = editor.get(attr, &record, false);
                    matches.find(text, matchers);
                    Ok(())
                },
                |record, &mut (ref mut editor, ref matches), vars| {
                    if let Some(keep) = filter {
                        if (matches.num_matches() > 0) ^ keep {
                            if let Some(ref mut f) = dropped_file {
                                f.write(&record, vars)?;
                            }
                            return Ok(true);
                        }
                    }

                    if let Some(rep) = replacement.as_ref() {
                        editor.edit_with_val(attr, &record, true, |text, out| {
                            match_vars.set_with(
                                record,
                                matches,
                                &mut vars.mut_data().symbols,
                                text,
                            )?;

                            replacement_text.clear();
                            rep.compose(&mut replacement_text, vars.symbols());

                            let pos = matches
                                .matches_iter(0, 0)
                                .filter_map(|m| m)
                                .map(|m| (m.start, m.end));
                            replace_iter(text, &replacement_text, out, pos);

                            Ok::<(), ::error::CliError>(())
                        })?;
                    } else {
                        let text = editor.get(attr, &record, true);
                        match_vars.set_with(record, matches, &mut vars.mut_data().symbols, text)?;
                    }

                    writer.write(&editor.rec(&record), vars)?;
                    Ok(true)
                },
            )?;
            Ok(())
        },
    )?;
    Ok(())
}

fn analyse_patterns<S>(
    patterns: &[(S, S)],
    algo_override: Option<Algorithm>,
    typehint: Option<&str>,
    ambig_override: Option<bool>,
    regex: bool,
    dist: u16,
    verbose: bool,
) -> CliResult<(SeqType, Vec<(Algorithm, bool)>)>
where
    S: AsRef<str> + Display,
{
    use std::collections::HashSet;
    let mut ambig_seqs = vec![];

    let (unique_seqtypes, out): (HashSet<SeqType>, Vec<(Algorithm, bool)>) = patterns
        .iter()
        .map(|&(ref name, ref pattern)| {
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
                if is_ambig || pattern.as_ref().len() > 64 {
                    Ukkonen
                } else {
                    Myers
                }
            } else {
                Exact
            };

            // override with user choice
            if let Some(a) = algo_override {
                algorithm = a;
                if a != Ukkonen && is_ambig {
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

fn align_score_eq(a: u8, b: u8) -> i32 {
    if a == b {
        1
    } else {
        -1
    }
}

fn align_score_ambig_dna(patt: u8, search: u8) -> i32 {
    AsymmIdentityDnaMatrix::get(search, patt) as i32
}

fn align_score_ambig_protein(patt: u8, search: u8) -> i32 {
    if search == patt || patt == b'X' {
        1
    } else {
        -1
    }
}

fn unit_cost_ambig_dna(patt: u8, search: u8) -> u32 {
    (AsymmIdentityDnaMatrix::get(search, patt) == -1) as u32
}

fn unit_cost_ambig_protein(patt: u8, search: u8) -> u32 {
    (patt != search || patt != b'X') as u32
}

fn get_matcher<'a>(
    pattern: &str,
    algorithm: Algorithm,
    ambig: bool,
    o: &MatchOpts,
) -> CliResult<Box<Matcher + Send + 'a>> {
    Ok(match algorithm {
        Exact =>
          Box::new(ExactMatcher::new(pattern.as_bytes())),
        Regex =>
          // TODO: string regexes for ID/desc
          Box::new(BytesRegexMatcher::new(pattern, o.has_groups)?),
        Myers =>
          Box::new(MyersMatcher::new(
            pattern.as_bytes(), o.max_dist as u8, o.needs_alignment, o.sorted, o.group_pos,
            &align_score_eq
          )?),
        Ukkonen => {
            if ambig {
                if o.seqtype == SeqType::DNA {
                    Box::new(UkkonenMatcher::new(
                      pattern.as_bytes(), o.max_dist as u8,
                      o.needs_alignment, o.sorted, o.group_pos,
                      &unit_cost_ambig_dna, &align_score_ambig_dna
                    )?)

                } else {
                    // relies on correct detection of the sequence type. Invalid amino acids
                    // are not recognized and will positively match against X
                    Box::new(UkkonenMatcher::new(
                      pattern.as_bytes(), o.max_dist as u8,
                      o.needs_alignment, o.sorted, o.group_pos,
                      &unit_cost_ambig_protein, &align_score_ambig_protein
                    )?)
                  }
            } else {
                Box::new(UkkonenMatcher::new(
                  pattern.as_bytes(), o.max_dist as u8,
                  o.needs_alignment, o.sorted, o.group_pos,
                  &unit_cost, &align_score_eq
                )?)
            }
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
