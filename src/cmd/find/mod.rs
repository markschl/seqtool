use std::fs::File;
use std::io::{BufWriter, Write};

use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::helpers::util::replace_iter;
use crate::io::{RecordAttr, RecordEditor};
use crate::var::{varstring, VarProvider};

use super::shared::seqtype::SeqType;

mod cli;
mod helpers;
mod matcher;
mod matches;
mod vars;

pub use self::cli::*;
use self::matches::*;
pub use self::vars::*;

#[derive(Debug)]
struct MatchOpts {
    has_groups: bool,
    bounds_needed: bool,
    sorted: bool,
    max_dist: usize,
    seqtype: SeqType,
}

pub fn run(mut cfg: Config, args: &FindCommand) -> CliResult<()> {
    // assemble all settings

    let verbose = args.common.general.verbose;
    // search options
    let patterns = args.patterns.to_vec();
    let regex = args.search.regex;
    let max_dist = args.search.dist;
    let sorted = args.search.in_order;
    let typehint = args.search.seqtype;
    let num_threads = args.search.threads;
    let no_ambig = args.search.no_ambig;
    let algo_override = args.search.algo;
    // search range
    let bounds = args
        .search_range
        .rng
        .map(|rng| rng.adjust(false, false))
        .transpose()?;
    let max_shift = if let Some(n) = args.search_range.max_shift_l {
        Some(Shift::Start(n))
    } else {
        args.search_range.max_shift_r.map(Shift::End)
    };
    // what should be searched?
    let attr = if args.attr.id {
        RecordAttr::Id
    } else if args.attr.desc {
        RecordAttr::Desc
    } else {
        RecordAttr::Seq
    };
    // search "actions"
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
    let dropped_file = args.action.dropped.clone();
    let replacement = args.action.rep.as_deref();

    // Obtain a sequence type and search algorithm for each pattern
    // (based on heuristic and/or CLI args)
    let (seqtype, algorithms) = helpers::analyse_patterns(
        &patterns,
        algo_override,
        typehint,
        no_ambig,
        regex,
        max_dist,
        verbose,
    )?;

    let mut format_writer = cfg.get_format_writer()?;

    // Parse possible replacement strings.
    // These can contain variables/expressions.
    let replacement = replacement
        .map(|text| {
            cfg.with_command_vars::<FindVars, _>(|v, _| {
                let match_vars = v.unwrap();
                // For pattern replacement, all hits for group 0 (the full hit) must
                // be known.
                // TODO: API is somehow awkward
                match_vars.register_all(0);
                Ok(())
            })?;
            let (s, _) = cfg.build_vars(|b| varstring::VarString::parse_register(text, b, true))?;
            Ok::<_, CliError>(s)
        })
        .transpose()?;

    // Validate and determine requirements to build the configuration.
    // note: Config::with_command_vars() is called a second time here to avoid borrowing issues
    let (match_cfg, opts) = cfg.with_command_vars::<FindVars, _>(|match_vars, _| {
        let match_vars = match_vars.unwrap();
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

    // More things needed during the search
    // intermediate buffer for replacement text
    let mut replacement_text = vec![];

    let (pattern_names, patterns): (Vec<_>, Vec<_>) = patterns.into_iter().unzip();
    // buffered writer for dropped records
    let mut dropped_file = if let Some(f) = dropped_file.as_ref() {
        Some(BufWriter::new(File::create(f)?))
    } else {
        None
    };

    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel_init(
            num_threads,
            || {
                // initialize matchers (one per record set)
                algorithms
                    .iter()
                    .zip(&patterns)
                    .map(|(&(algo, is_ambig), patt)| {
                        helpers::get_matcher(patt, algo, attr, is_ambig, &opts)
                    })
                    .collect::<CliResult<Vec<_>>>()
            },
            || {
                // initialize per-record data
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
                // do the searching in the worker threads
                let text = editor.get(attr, &record, false);
                // update the `Matches` object with the results reported by every `Matcher`
                matches.find(text, matchers)
            },
            |record, &mut (ref mut editor, ref matches), ctx| {
                // handle results in main thread, write output

                // update variables (if any) with search results obtained in the 'work' closure
                ctx.command_vars::<FindVars, _>(|match_vars, symbols| {
                    if let Some(_match_vars) = match_vars {
                        let text = editor.get(attr, &record, true);
                        _match_vars.set_with(record, matches, symbols, text)?;
                    }
                    Ok(())
                })?;

                // fill in replacements (if necessary)
                if let Some(rep) = replacement.as_ref() {
                    editor.edit_with_val(attr, &record, true, |text, out| {
                        // assemble replacement text
                        replacement_text.clear();
                        rep.compose(&mut replacement_text, &ctx.symbols, record)?;
                        // replace all occurrences of the pattern
                        let pos = matches
                            .matches_iter(0, 0)
                            .flatten()
                            .map(|m| (m.start, m.end));
                        replace_iter(text, pos, out, |o, _, _| o.write_all(&replacement_text))
                            .unwrap();
                        Ok::<(), CliError>(())
                    })?;
                }

                // keep / exclude
                if let Some(keep) = filter {
                    if matches.has_matches() ^ keep {
                        if let Some(ref mut f) = dropped_file {
                            // we don't write the edited record, since there are no hits to report
                            format_writer.write(&record, f, ctx)?;
                        }
                        return Ok(true);
                    }
                }

                // write non-excluded to output
                let edited_rec = editor.record(&record);
                format_writer.write(&edited_rec, io_writer, ctx)?;
                Ok(true)
            },
        )?;
        Ok(())
    })
}
