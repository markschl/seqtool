use std::io::Write;
use std::mem;
use std::ops::{Deref, DerefMut};

use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{value::SimpleValue, DefaultHashMap as HashMap};
use crate::var::varstring::VarString;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Count' command options")]
pub struct CountCommand {
    /// Count sequences for each unique value of the given category.
    /// Can be a single variable/function such as 'filename', 'desc' or 'attr(name)',
    /// or a composed key such as '{filename}_{meta(species)}'.
    /// The `-k/--key` argument can be specified multiple times, in which case
    /// there will be multiple category columns, one per key.
    #[arg(short, long)]
    key: Vec<String>,

    /// Maximum number of categories to count before aborting with an error.
    /// This limit is a safety measure to prevent memory exhaustion.
    /// Usually, a very large number of categories is not intended and may
    /// happen if continuous numbers are not categorized with the
    /// `bin(<num>, <interval>)` function.
    #[arg(short = 'l', long, default_value_t = 1000000)]
    category_limit: usize,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &CountCommand) -> CliResult<()> {
    if args.key.is_empty() {
        count_simple(cfg)
    } else {
        count_categorized(cfg, &args.key, args.category_limit)
    }
}

fn count_simple(cfg: Config) -> CliResult<()> {
    // run counting without any variable processing
    cfg.with_io_writer(|writer, mut cfg| {
        let mut n = 0;
        cfg.read(|_, _| {
            n += 1;
            Ok(true)
        })?;
        // TODO: line terminator?
        writeln!(writer, "{}", n)?;
        Ok(())
    })?;
    Ok(())
}

fn count_categorized<S>(mut cfg: Config, keys: &[S], category_limit: usize) -> CliResult<()>
where
    S: AsRef<str>,
{
    // register variables/functions:
    // tuples of (varstring, text buffer)
    let mut var_keys: Vec<(VarString, Vec<u8>)> = keys
        .iter()
        .map(|k| {
            cfg.build_vars(|b| {
                let (key, _) = VarString::parse_register(k.as_ref(), b, true)?;
                Ok((key, Vec::new()))
            })
        })
        .collect::<Result<_, String>>()?;
    // this is the final hashmap key, only cloned if needed
    // let mut key = vec![SimpleValue::None; var_keys.len()];
    let mut key = Key::new(var_keys.len());

    // hashmap holding the counts
    let mut counts = HashMap::default();

    // count the records
    cfg.read(|record, ctx| {
        for ((varstring, text_buf), value) in var_keys.iter_mut().zip(key.iter_mut()) {
            varstring.simple_value(value, text_buf, &ctx.symbols, record)?
        }
        if let Some(v) = counts.get_mut(&key) {
            *v += 1;
        } else if counts.len() <= category_limit {
            counts.insert(key.clone(), 1usize);
        } else {
            return fail!(
                "Reached the limit of {} categories while counting records, aborting.{} \
                To count more categories, raise the limit using `-l/--category-limit`.",
                category_limit,
                if counts
                    .keys()
                    .any(|k| k.iter().any(|v| matches!(v, SimpleValue::Number(_))))
                {
                    " Consider using the function 'bin(<number>, <interval>)' to group \
                    numeric values into regular intervals."
                } else {
                    ""
                }
            );
        }
        Ok(true)
    })?;

    // sort the keys
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort();

    // write to output
    let mut buf = Vec::new();
    let mut prev_buf = Vec::new();
    let mut count = 0;
    cfg.with_io_writer(|writer, _cfg| {
        for (keys, n) in sorted {
            // write the keys to the current buffer
            buf.clear();
            for key in keys.iter() {
                key.write(&mut buf)?;
                write!(&mut buf, "\t")?;
            }
            // If the formatted key is distinct from the previous one,
            // report the count.
            // Otherwise, there must be floating-point numbers resulting in the
            // same output when printed, so we just accumulate the count
            // dbg!((std::str::from_utf8(&prev_buf), std::str::from_utf8(&buf), count, n));
            if count == 0 {
                // in the first iteration, prev_buf is empty, so we can't
                // yet compare the two
                mem::swap(&mut buf, &mut prev_buf);
                count += n;
            } else if buf != prev_buf {
                // keys are distinct -> report the count of the previous key
                // (current one still not written since we don't know if it is
                // distinct yet) -> assign 'count = n')
                writer.write_all(&prev_buf)?;
                writeln!(writer, "{}", count)?;
                mem::swap(&mut buf, &mut prev_buf);
                count = n;
            } else {
                // keys are not distinct -> just accumulate
                count += n;
            }
        }
        // write the last line
        writer.write_all(&prev_buf)?;
        writeln!(writer, "{}", count)?;
        Ok(())
    })
}

/// Hashmap key for categorized counting.
///
/// Ther performance improvement if not allocating a single value seems mostly small
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
enum Key {
    One(SimpleValue),
    More(Box<[SimpleValue]>),
}

impl Key {
    fn new(size: usize) -> Self {
        use SimpleValue as SV;
        match size {
            1 => Self::One(SV::None),
            _ => Self::More(vec![SV::None; size].into_boxed_slice()),
        }
    }
}

impl Deref for Key {
    type Target = [SimpleValue];
    fn deref(&self) -> &Self::Target {
        use Key::*;
        match self {
            One(v) => std::slice::from_ref(v),
            More(v) => v,
        }
    }
}

impl DerefMut for Key {
    fn deref_mut(&mut self) -> &mut Self::Target {
        use Key::*;
        match self {
            One(v) => std::slice::from_mut(v),
            More(v) => v,
        }
    }
}
