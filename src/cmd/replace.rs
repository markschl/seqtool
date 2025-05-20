use std::borrow::ToOwned;
use std::str;

use clap::{value_parser, Parser};
use memchr::memmem::find_iter;

use crate::cli::CommonArgs;
use crate::error::CliResult;
use crate::helpers::replace::replace_iter;
use crate::io::{RecordAttr, RecordEditor};
use crate::Config;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Replace' command options")]
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

    /// Interpret pattern as a regular expression.
    /// Unicode characters are supported when searching in IDs/descriptions,
    /// but not for sequence searches.
    #[arg(short, long)]
    regex: bool,

    /// Number of threads
    #[arg(short, long, value_name = "N", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    threads: u32,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(mut cfg: Config, args: ReplaceCommand) -> CliResult<()> {
    // what should be replaced?
    let attr = if args.id {
        RecordAttr::Id
    } else if args.desc {
        RecordAttr::Desc
    } else {
        RecordAttr::Seq
    };
    let pattern = &args.pattern;
    let replacement = args.replacement.as_bytes();
    let has_backrefs = replacement.contains(&b'$');
    let regex = args.regex;
    let num_threads = args.threads;

    let replacer = get_replacer(pattern, regex, has_backrefs)?;

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
                format_writer.write(&editor.record(&record), io_writer, ctx)?;
                Ok(true)
            },
        )
    })?;
    Ok(())
}

trait Replacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()>;
}

struct BytesReplacer(Vec<u8>);

impl Replacer for BytesReplacer {
    fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
        let matches = find_iter(text, &self.0).map(|start| (start, start + self.0.len()));
        replace_iter(text, replacement, matches, out).unwrap();
        Ok(())
    }
}

macro_rules! regex_replacer_impl {
    ($name:ident, $regex:ty, $to_string:expr, $to_bytes:expr) => {
        struct $name {
            re: $regex,
            has_backrefs: bool,
        }

        impl $name {
            fn new(pattern: &str, has_backrefs: bool) -> CliResult<Self> {
                Ok(Self {
                    re: <$regex>::new(pattern)?,
                    has_backrefs,
                })
            }
        }

        impl Replacer for $name {
            #[allow(clippy::redundant_closure_call)]
            fn replace(&self, text: &[u8], replacement: &[u8], out: &mut Vec<u8>) -> CliResult<()> {
                let search_text = $to_string(text)?;
                if !self.has_backrefs {
                    let matches = self.re.find_iter(search_text).map(|m| (m.start(), m.end()));
                    replace_iter(text, replacement, matches, out).unwrap();
                } else {
                    // requires allocations
                    let repl_text = $to_string(replacement)?;
                    let replaced = self.re.replace_all(search_text, repl_text);
                    out.extend_from_slice($to_bytes(replaced.as_ref()));
                }
                Ok(())
            }
        }
    };
}

cfg_if::cfg_if! {
    if #[cfg(feature = "regex-fast")] {
        regex_replacer_impl!(BytesRegexReplacer, regex::bytes::Regex, Ok::<_, crate::error::CliError>, |s| s);
    } else {
        // TODO: no way to operate on byte slices (although it might be added according to regex_lite docs)
        regex_replacer_impl!(BytesRegexReplacer, regex_lite::Regex, |t| std::str::from_utf8(t), str::as_bytes);
    }
}

fn get_replacer(
    pattern: &str,
    regex: bool,
    has_backrefs: bool,
) -> CliResult<Box<dyn Replacer + Sync>> {
    if regex {
        Ok(Box::new(BytesRegexReplacer::new(pattern, has_backrefs)?))
    } else {
        Ok(Box::new(BytesReplacer(pattern.as_bytes().to_owned())))
    }
}
