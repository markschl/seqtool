use std::fmt::{self, Debug, Write};

use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{value::SimpleValue, DefaultHashMap as HashMap};
use crate::io::Record;
use crate::var::{symbols::SymbolTable, varstring, VarBuilder};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct CountCommand {
    /// Summarize over a variable/function or a string containing variables.
    /// Multiple -k/--key arguments can be supplied to sumarize over multiple
    /// categories.
    /// Numeric values are summarized in intervals of 1. To change, specify
    /// --key 'n:<interval>:<key>'. Example: '--key n:10:attr(size)'.
    /// The 'n:' prefix stands for 'numeric' and can also be supplied if a
    /// text key (e.g. a field from an associated list or a header attribute)
    /// should be interpreted as numeric.
    /// The interval can be omitted, example (using default interval of 1):
    /// '--key n:attr(size)'
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
        count_simple(cfg)
    } else {
        count_categorized(cfg, &args.key, !args.no_int)
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
                        width: interval,
                        precision,
                    }),
                    &s[3 + end + 1..s.len()],
                );
            }
        }
        return (
            Some(Interval {
                width: default_interval,
                precision: default_precision,
            }),
            &s[2..s.len()],
        );
    }
    (None, s)
}

#[derive(Default, Clone)]
struct Interval {
    pub width: f64,
    pub precision: usize,
}

impl Interval {
    fn new(interval: f64, precision: usize) -> Self {
        Self {
            width: interval,
            precision,
        }
    }

    pub fn write<W: fmt::Write>(&self, num: f64, mut out: W) -> fmt::Result {
        write!(
            out,
            "({0:.2$},{1:.2$}]",
            num * self.width,
            (num + 1.) * self.width,
            self.precision
        )
    }
}

struct VarKey {
    key: varstring::VarString,
    value: SimpleValue,
    text_buf: Vec<u8>,
    interval: Interval,
    is_discrete: bool,
    force_numeric: bool,
}

impl VarKey {
    fn from_str(s: &str, builder: &mut VarBuilder) -> Result<Self, String> {
        let (interval, key) = parse_key(s, 1., 0);
        Ok(Self {
            key: varstring::VarString::parse_register(key, builder, true)?.0,
            value: SimpleValue::None,
            text_buf: Vec::new(),
            interval: interval.clone().unwrap_or(Interval::new(1., 0)),
            is_discrete: true,
            force_numeric: interval.is_some(),
        })
    }

    fn categorize(
        &mut self,
        symbols: &SymbolTable,
        record: &dyn Record,
        out: &mut Category,
    ) -> CliResult<()> {
        self.key.simple_value(
            &mut self.value,
            &mut self.text_buf,
            symbols,
            record,
            self.force_numeric,
        )?;
        match &self.value {
            SimpleValue::Number(val) => {
                if !val.is_nan() {
                    let v = val / self.interval.width;
                    if v.fract() != 0. {
                        self.is_discrete = false;
                    }
                    *out = Category::Num(v.floor() as i64);
                } else {
                    *out = Category::NaN;
                }
            }
            SimpleValue::Text(val) => {
                if let Category::Text(ref mut v) = *out {
                    v.clear();
                    v.extend_from_slice(val);
                } else {
                    *out = Category::Text(val.to_vec());
                }
            }
            SimpleValue::None => *out = Category::NA,
        }
        Ok(())
    }

    fn interval(&self) -> (Interval, bool) {
        (self.interval.clone(), self.is_discrete)
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
        interval: Interval,
        is_discrete: bool,
        print_intervals: bool,
    ) -> CliResult<()> {
        match self {
            Category::Text(ref s) => write!(out, "{}", std::str::from_utf8(s)?)?,
            Category::Num(n) => {
                if print_intervals && !is_discrete {
                    interval.write(*n as f64, out)?;
                } else {
                    write!(
                        out,
                        "{0:.1$}",
                        (*n as f64) * interval.width,
                        interval.precision
                    )?;
                }
            }
            Category::NaN => write!(out, "NaN")?,
            Category::NA => write!(out, "N/A")?,
        }
        Ok(())
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

fn count_categorized<S>(mut cfg: Config, keys: &[S], print_intervals: bool) -> CliResult<()>
where
    S: AsRef<str>,
{
    // register variables & parse types
    let mut var_keys: Vec<_> = keys
        .iter()
        .map(|k| cfg.build_vars(|b| VarKey::from_str(k.as_ref(), b)))
        .collect::<Result<_, String>>()?;

    // count the records
    let mut counts = HashMap::default();
    // reusable key that is only cloned when not present in the hash map
    let mut key = vec![Category::NA; var_keys.len()];

    // count the records
    cfg.read(|record, ctx| {
        for (key, cat) in var_keys.iter_mut().zip(&mut key) {
            key.categorize(&ctx.symbols, record, cat)?;
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
            let (int, is_discrete) = key.interval();
            cat.to_text(&mut row, int, is_discrete, print_intervals)?;
            write!(&mut row, "\t")?;
        }
        // write the count
        // TODO: line terminator?
        write!(&mut row, "{}", count)?;
        println!("{}", row);
    }
    Ok(())
}
