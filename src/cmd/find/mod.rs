use std::collections::HashMap;
use std::str;

use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::helpers::seqtype::SeqType;
use crate::helpers::util::replace_iter;
use crate::io::{output::writer::Writer, RecordEditor, SeqAttr};
use crate::var::{varstring, VarHelp, VarProvider};

mod helpers;
mod matcher;
mod matches;
mod opts;
mod vars;

use self::helpers::Algorithm;
use self::matches::*;
pub use self::opts::*;
use self::vars::*;

struct MatchOpts {
    has_groups: bool,
    bounds_needed: bool,
    sorted: bool,
    max_dist: usize,
    seqtype: SeqType,
}

pub fn run(cfg: Config, args: &FindCommand) -> CliResult<()> {
    let max_dist = args.search.dist;
    let sorted = args.search.in_order;
    let regex = args.search.regex;
    let no_ambig = args.search.no_ambig;
    let verbose = args.common.general.verbose;

    let attr = if args.attr.id {
        SeqAttr::Id
    } else if args.attr.desc {
        SeqAttr::Desc
    } else {
        SeqAttr::Seq
    };

    let filter = if args.action.filter {
        Some(true)
    } else if args.action.exclude {
        if args.action.filter {
            return fail!("-f/--filter and -e/--exclude cannot both be specified");
        }
        Some(false)
    } else {
        None
    };

    let num_threads = args.search.threads;

    let pattern = &args.pattern;
    let patterns = if !pattern.starts_with("file:") {
        vec![("<pattern>".to_string(), pattern.to_string())]
    } else {
        helpers::read_pattern_file(&pattern[5..])?
    };

    let typehint = args.search.seqtype;

    //let replace_num = args.get_str("--match-num");
    let replacement = &args.action.rep;

    let bounds = args
        .search_range
        .rng
        .map(|rng| rng.adjust(false, false))
        .transpose()?;

    let max_shift = if let Some(n) = args.search_range.max_shift_l {
        Some(Shift::Start(n))
    } else if let Some(n) = args.search_range.max_shift_r {
        Some(Shift::End(n))
    } else {
        None
    };

    let dropped_file = args.action.dropped.clone();

    // override algorithm for testing
    let algo_override = Algorithm::from_str(&args.search.algo);

    ///// option parsing end

    // determine sequence type for each pattern
    let (seqtype, algorithms) = helpers::analyse_patterns(
        &patterns,
        algo_override,
        typehint,
        no_ambig,
        regex,
        max_dist,
        verbose,
    )?;

    let v = Box::new(FindVars::new(patterns.len()));
    cfg.with_vars(Some(v), |vars| {
        // run
        cfg.writer_with(vars, |writer, mut vars| {

            // make sure all hits for group 0 are collected (group 0 is always searched)
            // API is somehow awkward
            let replacement = if let Some(r) = replacement {
                vars.custom_mod::<FindVars, _>(|v, _| {
                    v.unwrap().register_all(0);
                    Ok(())
                })?;
                let s = vars.build(|b| {
                    varstring::VarString::var_or_composed(&r, b)
                })?;
                Some(s)
            } else {
                None
            };

            let (match_cfg, opts) = vars.custom_mod::<FindVars, _>(|match_vars, _| {
                let match_vars = match_vars.unwrap();   // FindVars::has_vars() always returns true -> always present

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
                        .map(|(&(algo, is_ambig), patt)| helpers::get_matcher(patt, algo, is_ambig, &opts))
                        .collect::<CliResult<Vec<_>>>()
                },
                || {
                    // initialize per-sequence record data
                    let editor = Box::<RecordEditor>::default();
                    let matches = Box::new(Matches::new(
                        &pattern_names,
                        match_cfg.clone(),
                        bounds,
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
                    vars.custom_mod::<FindVars, _>(|match_vars, symbols| {
                        let match_vars = match_vars.unwrap();
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
