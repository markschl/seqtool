use std::fmt::Debug;
use std::fmt::Write;
use std::mem;

use csv;

use cfg;
use error::CliResult;
use fxhash::FxHashMap;
use opt;
use var::varstring;

static USAGE: &'static str = concat!(
    "
This command counts the number of sequences and prints the number to STDOUT. Advanced
grouping of sequences is possible by supplying or more key strings containing
variables (-k).

Usage:
    st count [options] [-l <list>...] [-k <key>...] [<input>...]
    st count (-h | --help)

Options:
    -k, --key <key>     Summarize over a variable key or a string containing variables.
                        For numeric key insert 'n:' before. Values are counted
                        in intervals of 1. To change, specify 'n:<interval>:<key>'.
                        Example: 'n:10:{s:seqlen}'
    -n, --no-int        Don't print intervals when using the 'n:<interval>:<key> syntax',
                        instead only upper limits (e.g. '5' instead of '(1,5]')
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args(&args)?;

    let keys = args.get_vec("--key");
    let print_intervals = !args.get_bool("--no-int");

    if keys.is_empty() {
        count_simple(&cfg)
    } else {
        count_categorized(&cfg, &keys, print_intervals)
    }
}

fn count_simple(cfg: &cfg::Config) -> CliResult<()> {
    cfg.io_writer(|writer, _| {
        let mut n = 0;

        cfg.read_sequential(|_| {
            n += 1;
            Ok(true)
        })?;

        writeln!(writer, "{}", n)?;

        Ok(())
    })?;
    Ok(())
}

fn count_categorized(cfg: &cfg::Config, keys: &[&str], print_intervals: bool) -> CliResult<()> {
    cfg.io_writer(|writer, mut vars| {
        // register variables & parse types
        let var_keys: Vec<_> = keys
            .iter()
            .map(|k| {
                let (interval, key) = parse_key(k, 1., 0);
                let var_key = vars.build(|b| varstring::VarString::var_or_composed(key, b))?;
                Ok((var_key, interval))
            })
            .collect::<CliResult<_>>()?;
        // count
        let mut counts = FxHashMap::default();

        // vec of reusable strings for generating the key values
        let mut values = vec![vec![]; var_keys.len()];
        // reusable key that is only cloned when not present in the hash map
        let mut key = vec![(Category::Text(vec![]), false); var_keys.len()];

        cfg.read_sequential_var(&mut vars, |_, vars| {
            for (
                (&(ref key, ref interval), ref mut value),
                &mut (ref mut cat, ref mut is_different),
            ) in var_keys.iter().zip(&mut values).zip(&mut key)
            {
                if let Some(&(int, _)) = interval.as_ref() {
                    if let Some(v) = key.get_float(vars.symbols())? {
                        if !v.is_nan() {
                            let v = v / int as f64;
                            let f = v.floor();
                            if v != f {
                                *is_different = true;
                            }
                            *cat = Category::Num(f as i64);
                        } else {
                            *cat = Category::NaN;
                        }
                    } else {
                        *cat = Category::NA;
                    }
                } else {
                    value.clear();
                    key.compose(value, vars.symbols());
                    if let Category::Text(ref mut v) = *cat {
                        mem::swap(v, value);
                    } else {
                        *cat = Category::Text(value.clone());
                    }
                }
            }

            // cannot use Entry API because this would require the key to be cloned
            if let Some(v) = counts.get_mut(&key) {
                *v += 1;
                return Ok(true);
            }
            counts.insert(key.clone(), 1);

            Ok(true)
        })?;

        // sort
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort();
        // write
        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(writer);

        let mut record = vec![String::new(); var_keys.len() + 1];
        for (ref keys, count) in sorted {
            for ((ref mut field, &(ref c, is_different)), &(_, ref interval)) in
                record.iter_mut().zip(keys).zip(&var_keys)
            {
                field.clear();
                match *c {
                    Category::Text(ref s) => field.push_str(::std::str::from_utf8(s)?),
                    Category::Num(n) => {
                        let &(int, precision) = &interval.unwrap();
                        if print_intervals && is_different {
                            write!(
                                field,
                                "({0:.2$},{1:.2$}]",
                                n as f64 * int,
                                (n + 1) as f64 * int,
                                precision
                            )?;
                        } else {
                            write!(field, "{0:.1$}", n as f64 * int, precision)?;
                        }
                    }
                    Category::NaN => field.push_str("NaN"),
                    Category::NA => field.push_str("N/A"),
                }
            }
            {
                let count_field = &mut record[var_keys.len()];
                count_field.clear();
                write!(count_field, "{}", count)?;
            }
            csv_writer.write_record(&record)?;
        }
        Ok(())
    })?;
    Ok(())
}

// returns (Option<interval>, actual_key)
fn parse_key(
    s: &str,
    default_interval: f64,
    default_precision: usize,
) -> (Option<(f64, usize)>, &str) {
    if s.len() >= 2 && &s[0..2] == "n:" {
        if let Some(end) = s.chars().skip(3).position(|c| c == ':') {
            let num = &s[2..3 + end];
            if let Ok(int) = num.parse() {
                let precision = num
                    .chars()
                    .position(|c| c == '.')
                    .map(|pos| num.len() - pos - 1)
                    .unwrap_or(0);
                return (Some((int, precision)), &s[3 + end + 1..s.len()]);
            }
        }
        return (Some((default_interval, default_precision)), &s[2..s.len()]);
    }
    (None, s)
}

#[derive(Debug, Hash, Eq, PartialOrd, Ord, PartialEq, Clone)]
enum Category<T: Debug> {
    Text(T),
    Num(i64),
    NaN,
    NA,
}
