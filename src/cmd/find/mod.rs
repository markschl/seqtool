use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::helpers::replace::replace_iter;
use crate::io::RecordEditor;
use crate::var::{modules::VarProvider, varstring::VarString};

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

pub fn run(mut cfg: Config, args: FindCommand) -> CliResult<()> {
    // parse all CLI options and initialize replacements
    let (search_config, search_opts, filter_opts) = args.parse(cfg.input_config()[0].1.seqtype)?;

    // add variable provider
    cfg.set_custom_varmodule(Box::new(FindVars::new(search_config)))?;

    // Parse replacement strings, which may contain variables/expressions.
    let replacement = search_opts
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
    let mut dropped_out = filter_opts
        .dropped_path
        .as_ref()
        .map(|f| cfg.new_output(f))
        .transpose()?;

    // finally, check check if there is anything to do,
    // and take `SearchConfig` back from the variable provider
    let mut search_config = cfg.with_command_vars(|match_vars, _| {
        let match_vars: &mut FindVars = match_vars.unwrap();
        if filter_opts.filter.is_none() && !match_vars.has_vars() && replacement.is_none() {
            return fail!(
                "Find command does nothing. Use -f/-e for filtering, --rep for replacing or \
                    -a for writing attributes."
            );
        }
        // now that all variables are registered, take the `search_config`
        // object back
        Ok::<_, String>(match_vars.take_config())
    })?;

    // intermediate buffer for replacement text
    let mut replacement_text = Vec::new();

    // also, in case of replacing, the position of all hits needs to be known
    if replacement.is_some() {
        search_config.require_n_hits(usize::MAX, RequiredDetail::Range);
        search_config.require_group(0); // full hit
    }

    // dbg!(&search_config, &search_opts, &filter_opts);

    // run the search
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel_init(
            search_opts.threads,
            // initialize matchers (one per record set)
            || get_matchers(&search_config, &search_opts),
            || {
                // initialize per-record data
                let editor: Box<RecordEditor> = Default::default();
                let matches = search_config.init_matches();
                (editor, matches)
            },
            |record, &mut (ref mut editor, ref mut matches), ref mut matchers| {
                // do the searching in the worker threads
                let text = editor.get(search_opts.attr, &record, false);
                // update the `Matches` object with the results reported by every `Matcher`
                matches
                    .collect_hits(text, matchers, &search_config)
                    .map_err(From::from)
            },
            |record, &mut (ref mut editor, ref matches), ctx| {
                // handle results in main thread, write output

                // update variables (if any) with search results obtained in the 'work' closure
                ctx.custom_vars(|match_vars: Option<&mut FindVars>, symbols| {
                    if let Some(_match_vars) = match_vars {
                        let text = editor.get(search_opts.attr, &record, true);
                        _match_vars.set_matches(record, &search_config, matches, symbols, text)?;
                    }
                    Ok::<_, String>(())
                })?;

                // fill in replacements (if necessary)
                if let Some(rep) = replacement.as_ref() {
                    editor.edit_with_val(search_opts.attr, &record, true, |text, out| {
                        // assemble replacement text
                        replacement_text.clear();
                        rep.compose(&mut replacement_text, &ctx.symbols, record)?;
                        // replace all occurrences of the pattern
                        let pos = search_config
                            .hits_iter(matches, 0, 0)
                            .map(|m| (m.start, m.end));
                        replace_iter(text, &replacement_text, pos, out).unwrap();
                        Ok::<(), CliError>(())
                    })?;
                }

                // keep / exclude
                if let Some(keep) = filter_opts.filter {
                    if matches.has_matches() ^ keep {
                        if let Some((d_writer, d_format_writer)) = dropped_out.as_mut() {
                            // we don't write the edited record, since there are no hits to report
                            d_format_writer.write(&record, d_writer, ctx)?;
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
    })?;
    if let Some((io_writer, _)) = dropped_out {
        io_writer.finish()?;
    }
    Ok(())
}
