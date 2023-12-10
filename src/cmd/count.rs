use std::fmt::{self, Debug, Write};
use std::mem;

use clap::Parser;
use fxhash::FxHashMap;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::val::TextValue;
use crate::io::Record;
use crate::opt::CommonArgs;
use crate::var::{symbols::SymbolTable, varstring, VarBuilder};

/// This command counts the number of sequences and prints the number to STDOUT. Advanced
/// grouping of sequences is possible by supplying or more key strings containing
/// variables (-k).
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct CountCommand {
    /// Summarize over a variable/function or a string containing variables.
    /// For numeric key insert 'n:' before. Values are counted
    /// in intervals of 1. To change, specify 'n:<interval>:<key>'.
    /// Example: 'n:10:{seqlen}'
    #[arg(short, long)]
    key: Vec<String>,

    /// Don't print intervals when using the 'n:<interval>:<key> syntax',
    /// instead only upper limits (e.g. '5' instead of '(1,5]')
    #[arg(short, long)]
    no_int: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &CountCommand) -> CliResult<()> {
    if args.key.is_empty() {
        count_simple(&cfg)
    } else {
        count_categorized(&cfg, &args.key, !args.no_int)
    }
}

// returns (Option<interval>, actual_key)
fn parse_key(s: &str, default_interval: f64, default_precision: usize) -> (Option<Interval>, &str) {
    if s.len() >= 2 && &s[0..2] == "n:" {
        if let Some(end) = s.chars().skip(3).position(|c| c == ':') {
            let num = &s[2..3 + end];
            if let Ok(interval) = num.parse() {
                let precision = num
                    .chars()
                    .position(|c| c == '.')
                    .map(|pos| num.len() - pos - 1)
                    .unwrap_or(0);
                return (
                    Some(Interval {
                        interval,
                        precision,
                    }),
                    &s[3 + end + 1..s.len()],
                );
            }
        }
        return (
            Some(Interval {
                interval: default_interval,
                precision: default_precision,
            }),
            &s[2..s.len()],
        );
    }
    (None, s)
}

#[derive(Default, Clone)]
struct Interval {
    pub interval: f64,
    pub precision: usize,
}

impl Interval {
    pub fn write<W: fmt::Write>(&self, num: f64, mut out: W) -> fmt::Result {
        write!(
            out,
            "({0:.2$},{1:.2$}]",
            num * self.interval,
            (num + 1.) * self.interval,
            self.precision
        )
    }
}

struct VarKey {
    key: varstring::VarString,
    val: TextValue,
    interval: Option<(Interval, bool)>,
}

impl VarKey {
    fn from_str(s: &str, builder: &mut VarBuilder) -> CliResult<Self> {
        let (interval, key) = parse_key(s, 1., 0);
        Ok(Self {
            key: varstring::VarString::var_or_composed(&key, builder)?,
            val: TextValue::default(),
            interval: interval.map(|i| (i, true)),
        })
    }

    fn categorize(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
        out: &mut Category,
    ) -> CliResult<()> {
        if let Some((int, ref mut is_discrete)) = self.interval.as_mut() {
            self.val.clear();
            if let Some(v) = self.key.get_float(symbols, record) {
                let v = v?;
                if !v.is_nan() {
                    let v = v / int.interval;
                    if v.fract() != 0. {
                        *is_discrete = false;
                    }
                    *out = Category::Num(v.floor() as i64);
                } else {
                    *out = Category::NaN;
                }
            } else {
                *out = Category::NA;
            }
        } else {
            let val = self.val.clear();
            self.key.compose(val, symbols, record);
            if let Category::Text(ref mut v) = *out {
                mem::swap(v, val);
            } else {
                *out = Category::Text(val.clone());
            }
        }
        Ok(())
    }

    fn interval(&self) -> Option<(Interval, bool)> {
        self.interval.clone()
    }
}

#[derive(Debug, Hash, Eq, PartialOrd, Ord, PartialEq, Clone)]
enum Category {
    Text(Vec<u8>),
    Num(i64),
    NaN,
    NA,
}

impl Category {
    fn to_text<W: fmt::Write>(
        &self,
        mut out: W,
        interval: Option<&(Interval, bool)>,
        print_intervals: bool,
    ) -> CliResult<()> {
        match self {
            Category::Text(ref s) => write!(out, "{}", std::str::from_utf8(s)?)?,
            Category::Num(n) => {
                let (int, is_discrete) = interval.unwrap();
                if print_intervals && !is_discrete {
                    int.write(*n as f64, out)?;
                } else {
                    write!(out, "{0:.1$}", (*n as f64) * int.interval, int.precision)?;
                }
            }
            Category::NaN => write!(out, "NaN")?,
            Category::NA => write!(out, "N/A")?,
        }
        Ok(())
    }
}

fn count_simple(cfg: &Config) -> CliResult<()> {
    // make sure --var-help is printed
    cfg.get_vars(None)?.finalize();
    // run counting without any variable processing
    cfg.io_writer(None, |writer, _| {
        let mut n = 0;
        cfg.read_simple(|_| {
            n += 1;
            Ok(true)
        })?;
        // TODO: line terminator?
        writeln!(writer, "{}", n)?;
        Ok(())
    })?;
    Ok(())
}

fn count_categorized<S>(cfg: &Config, keys: &[S], print_intervals: bool) -> CliResult<()>
where
    S: AsRef<str>,
{
    cfg.with_vars(None, |vars| {
        // register variables & parse types
        let mut var_keys: Vec<_> = keys
            .into_iter()
            .map(|k| vars.build(|b| VarKey::from_str(k.as_ref(), b)))
            .collect::<CliResult<_>>()?;

        // count the records
        let mut counts = FxHashMap::default();
        // reusable key that is only cloned when not present in the hash map
        let mut key = vec![Category::NA; var_keys.len()];
        cfg.read(vars, |record, vars| {
            for (key, cat) in var_keys.iter_mut().zip(&mut key) {
                key.categorize(vars.symbols(), record, cat)?;
            }

            // cannot use Entry API because this would require the key to be cloned
            if let Some(v) = counts.get_mut(&key) {
                *v += 1;
                return Ok(true);
            }
            counts.insert(key.clone(), 1);

            Ok(true)
        })?;

        // sort the keys
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort();

        let mut row = String::new();
        for (ref categories, count) in sorted {
            row.clear();
            // write the keys
            for (key, cat) in var_keys.iter().zip(categories) {
                cat.to_text(&mut row, key.interval().as_ref(), print_intervals)?;
                write!(&mut row, "\t")?;
            }
            // write the count
            // TODO: line terminator?
            write!(&mut row, "{}", count)?;
            println!("{}", row);
        }
        Ok(())
    })?;
    Ok(())
}
