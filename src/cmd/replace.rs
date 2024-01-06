use std::borrow::ToOwned;
use std::str;

use clap::{value_parser, Parser};
use memchr::Memchr;
use regex;

use crate::cli::CommonArgs;
use crate::error::CliResult;
use crate::helpers::twoway_iter::TwowayIter;
use crate::helpers::util::replace_iter;
use crate::io::{RecordEditor, SeqAttr};
use crate::Config;

/// Replaces the contents of sequence IDs, descriptions or sequences
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct ReplaceCommand {
    /// Search pattern
    pattern: String,

    /// Replacement string, cannot contain variables.
    replacement: String,

    /// Replace in IDs instead of sequences
    #[arg(short, long)]
    id: bool,

    /// Replace in descriptions
    #[arg(short, long)]
    desc: bool,

    /// Interpret <pattern> as regular expression
    #[arg(short, long)]
    regex: bool,

    /// Number of threads
    #[arg(short, long, value_name = "N", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    threads: u32,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &ReplaceCommand) -> CliResult<()> {
    // what should be replaced?
    let attr = if args.id {
        SeqAttr::Id
    } else if args.desc {
        SeqAttr::Desc
    } else {
        SeqAttr::Seq
    };

    let pattern = &args.pattern;
    let replacement = args.replacement.as_bytes();

    let has_backrefs = replacement.contains(&b'$');
    let regex = args.regex;
    let num_threads = args.threads;

    if regex {
        if attr == SeqAttr::Seq {
            let replacer = BytesRegexReplacer(regex::bytes::Regex::new(pattern)?, has_backrefs);
            run_replace(cfg, attr, replacement, replacer, num_threads)?;
        } else {
            let replacer = RegexReplacer(regex::Regex::new(pattern)?, has_backrefs);
            run_replace(cfg, attr, replacement, replacer, num_threads)?;
        }
    } else if pattern.len() == 1 {
        let replacer = SingleByteReplacer(pattern.as_bytes()[0]);
        run_replace(cfg, attr, replacement, replacer, num_threads)?;
    } else {
        let replacer = BytesReplacer(pattern.as_bytes().to_owned());
        run_replace(cfg, attr, replacement, replacer, num_threads)?;
    }
    Ok(())
}

fn run_replace<R: Replacer + Sync>(
    mut cfg: Config,
    attr: SeqAttr,
    replacement: &[u8],
    replacer: R,
    num_threads: u32,
) -> CliResult<()> {
    let mut format_writer = cfg.get_format_writer()?;
    cfg.with_io_writer(|io_writer, mut cfg| {
        cfg.read_parallel(
            num_threads - 1,
            |record, editor: &mut RecordEditor| {
                editor.edit_with_val(attr, &record, false, |text, out| {
                    replacer.replace(text, replacement, out)
                })
            },
            |record, editor, ctx| {
                format_writer.write(&editor.rec(&record), io_writer, ctx)?;
                Ok(true)
            },
        )
    })?;
    Ok(())
}

trait Replacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()>;
}

struct SingleByteReplacer(u8);

impl Replacer for SingleByteReplacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
        let matches = Memchr::new(self.0, text).map(|start| (start, start + 1));
        replace_iter(text, replacement, out, matches);
        Ok(())
    }
}

struct BytesReplacer(Vec<u8>);

impl Replacer for BytesReplacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
        let matches = TwowayIter::new(text, &self.0).map(|start| (start, start + self.0.len()));
        replace_iter(text, replacement, out, matches);
        Ok(())
    }
}

struct BytesRegexReplacer(regex::bytes::Regex, bool);

impl Replacer for BytesRegexReplacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
        if !self.1 {
            let matches = self.0.find_iter(text).map(|m| (m.start(), m.end()));
            replace_iter(text, replacement, out, matches);
        } else {
            // requires allocations
            let replaced = self.0.replace_all(text, replacement);
            out.extend_from_slice(&replaced);
        }
        Ok(())
    }
}

struct RegexReplacer(regex::Regex, bool);

impl Replacer for RegexReplacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
        let string = str::from_utf8(text)?;
        if !self.1 {
            let matches = self.0.find_iter(string).map(|m| (m.start(), m.end()));
            replace_iter(text, replacement, out, matches);
        } else {
            // requires allocations
            let replacement = str::from_utf8(replacement)?;
            let replaced = self.0.replace_all(string, replacement);
            out.extend_from_slice(replaced.as_bytes());
        }
        Ok(())
    }
}
