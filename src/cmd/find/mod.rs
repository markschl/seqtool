use std::cell::RefCell;
use std::fmt;

use crate::cli::Report;
use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::helpers::{replace::replace_iter, thread_local::with_mut_thread_local};
use crate::io::RecordEditor;
use crate::var::{modules::VarProvider, varstring::VarString};

use serde::Serialize;

pub mod ambig;
pub mod cli;
pub mod matcher;
pub mod matches;
pub mod opts;
pub mod vars;

pub use cli::FindCommand;
use matcher::get_matchers;
use opts::RequiredDetail;
pub use vars::FindVar;
use vars::FindVars;

thread_local! {
    static MATCHERS: RefCell<Option<Vec<Box<dyn matcher::Matcher + Send + Sync>>>> = RefCell::new(None);
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct FindStats {
    pub n_records: u64,
    pub n_matched: u64,
}

impl fmt::Display for FindStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Found {} of {} records", self.n_matched, self.n_records)
    }
}

pub fn run(mut cfg: Config, args: FindCommand) -> CliResult<Option<Box<dyn Report>>> {
    // parse all CLI options and initialize replacements
    let (search_config, opts) = args.parse(cfg.input_config[0].format.seqtype)?;

    // add variable provider
    cfg.set_custom_varmodule(Box::new(FindVars::new(search_config)))?;

    // Parse replacement strings, which may contain variables/expressions.
    let replacement = opts
        .replacement
        .as_deref()
        .map(|text| {
            let (s, _) = cfg.build_vars(|b| VarString::parse_register(text, b, false))?;
            Ok::<_, String>(s)
        })
        .transpose()?;

    // Object that formats the sequence output:
    // this registers all variables in header attribute / TSV fields
    let mut format_writer = cfg.get_format_writer()?;

    // Output for filtered records:
    // more variables may be registered here
    let mut dropped_out = opts
        .dropped_path
        .as_ref()
        .map(|f| cfg.new_output(f))
        .transpose()?;

    // finally, check check if there is anything to do,
    // and take `SearchConfig` back from the variable provider
    let mut search_config = cfg.with_custom_varmod(|v: &mut FindVars| {
        if opts.filter.is_none() && !v.has_vars() && replacement.is_none() {
            return fail!(
                "Find command does nothing. Use -f/-e for filtering, --rep for replacing or \
                    -a for writing attributes."
            );
        }
        // now that all variables are registered, take the `search_config`
        // object back
        Ok::<_, String>(v.take_config())
    })?;

    // intermediate buffer for replacement text
    let mut replacement_text = Vec::new();

    // also, in case of replacing, the positions of all hits need to be searched
    // and stored in the `Matches` object
    if replacement.is_some() {
        search_config.require_hit(None, 0, RequiredDetail::Range);
    }

    let matchers = get_matchers(&search_config)?;
    let matches = search_config.init_matches();

    let mut n_matched = 0;
    // dbg!(&search_config, &search_opts, &filter_opts);

    // run the search
    let stats = cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel_init(
            opts.threads,
            || {
                // initialize per-record data
                let editor: Box<RecordEditor> = Default::default();
                (editor, matches.clone())
            },
            |record, &mut (ref mut editor, ref mut matches)| {
                // do the searching in the worker threads
                let text = editor.get(opts.attr, &record, false);
                // update the `Matches` object with the results reported by every `Matcher`
                // TODO: thread local even used in single-threaded searching...
                //       however, this appears to have no measurable performance impact
                with_mut_thread_local(
                    &MATCHERS,
                    || matchers.clone(),
                    |_matchers| {
                        matches
                            .collect_hits(text, _matchers, &search_config)
                            .map_err(From::from)
                    },
                )
            },
            |record, &mut (ref mut editor, ref matches), ctx| {
                // handle results in main thread, write output

                // update variables (if any) with search results obtained in the 'work' closure
                ctx.with_custom_varmod(0, |v: &mut FindVars, sym| {
                    let text = editor.get(opts.attr, &record, true);
                    v.set_matches(record, &search_config, matches, sym, text)
                })
                .transpose()?;

                // fill in replacements (if necessary)
                if let Some(rep) = replacement.as_ref() {
                    editor.edit_with_val(opts.attr, &record, true, |text, out| {
                        // assemble replacement text
                        replacement_text.clear();
                        rep.compose(&mut replacement_text, ctx.symbols(), record)?;
                        // replace all occurrences of the pattern
                        let pos = search_config
                            .hits_iter(matches, 0, 0)
                            .map(|m| (m.start, m.end));
                        replace_iter(text, &replacement_text, pos, out).unwrap();
                        Ok::<(), CliError>(())
                    })?;
                }

                if matches.has_matches() {
                    n_matched += 1;
                }

                // keep / exclude
                if let Some(keep) = opts.filter
                    && matches.has_matches() ^ keep
                {
                    if let Some((d_writer, d_format_writer)) = dropped_out.as_mut() {
                        // we don't write the edited record, since there are no hits to report
                        d_format_writer.write(&record, d_writer, ctx)?;
                    }
                    return Ok(true);
                }

                // write non-excluded to output
                let edited_rec = editor.record(&record);
                format_writer.write(&edited_rec, io_writer, ctx)?;
                Ok(true)
            },
        )
    })?;
    if let Some((io_writer, _)) = dropped_out {
        io_writer.finish()?;
    }
    Ok(Some(Box::new(FindStats {
        n_records: stats.n_records,
        n_matched,
    })))
}
