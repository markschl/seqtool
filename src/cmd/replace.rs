use std::borrow::ToOwned;
use std::str;

use regex;
use memchr::Memchr;

use lib::twoway_iter::TwowayIter;
use error::CliResult;
use opt;
use io::{SeqAttr, RecordEditor};
use cfg;
use lib::util::replace_iter;


static USAGE: &'static str = concat!("
This command does fast search and replace for patterns in sequences
or ids/descriptions.

Usage:
    seqtool replace [options][-a <attr>...][-l <list>...] <pattern> <replacement> [<input>...]
    seqtool replace (-h | --help)
    seqtool replace --help-vars

Options:
    <replacement>       Replacement string, cannot contain variables.
    -i, --id            Replace in IDs instead of sequences
    -d, --desc          Replace in descriptions
    -r, --regex         Interpret <pattern> as regular expression
    -t, --threads <N>   Number of threads [default: 1]

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    // what should be replaced?
    let attr = if args.get_bool("--id") {
        SeqAttr::Id
    } else if args.get_bool("--desc") {
        SeqAttr::Desc
    } else {
        SeqAttr::Seq
    };

    let pattern = args.get_str("<pattern>");
    let replacement = args.get_str("<replacement>").as_bytes();

    let has_backrefs = replacement.contains(&b'$');
    let regex = args.get_bool("--regex");
    let num_threads = args.thread_num()?;

    if regex {
        if attr == SeqAttr::Seq {
            let replacer = BytesRegexReplacer(regex::bytes::Regex::new(pattern)?, has_backrefs);
            run_replace(&cfg, attr, replacement, replacer, num_threads)?;
        } else {
            let replacer = RegexReplacer(regex::Regex::new(pattern)?, has_backrefs);
            run_replace(&cfg, attr, replacement, replacer, num_threads)?;
        }
    } else {
        if pattern.len() == 1 {
            let replacer = SingleByteReplacer(pattern.as_bytes()[0]);
            run_replace(&cfg, attr, replacement, replacer, num_threads)?;
        } else {
            let replacer = BytesReplacer(pattern.as_bytes().to_owned());
            run_replace(&cfg, attr, replacement, replacer, num_threads)?;
        }
    }
    Ok(())
}

fn run_replace<R: Replacer + Sync>(
    cfg: &cfg::Config,
    attr: SeqAttr,
    replacement: &[u8],
    replacer: R,
    num_threads: u32,
) -> CliResult<()> {
    cfg.writer(|writer, mut vars| {
        cfg.var_parallel::<_, _, RecordEditor>(
            &mut vars,
            num_threads - 1,
            |record, editor| {
                editor.edit_with_val(attr, &record, false, |text, out| {
                    replacer.replace(text, replacement, out)
                })
            },
            |record, editor, vars| {
                writer.write(&editor.rec(&record), vars)?;
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
