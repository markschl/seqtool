use std::cmp::min;
use std::io::{self, Write};

use bit_vec::BitVec;
use byteorder::{BigEndian, WriteBytesExt};
use rand::distributions::Uniform;
use rand::prelude::*;

use crate::config;
use crate::error::CliResult;
use crate::io::output::Writer;
use crate::opt;
use crate::var::*;

pub static USAGE: &str = concat!(
    "
Return a random subset of sequences.

Usage:
    st sample [options][-a <attr>...][-l <list>...] [<input>...]
    st sample (-h | --help)
    st sample --help-vars

Options:
    -f, --frac <frac>   Randomly select with probability f returning the given
                        fraction of sequences on average.
    -n, --num-seqs <n>  Randomly selects exactly n records. Does not work with
                        STDIN because records have to be counted before.
    -s, --seed <s>      Use this seed to make the sampling reproducible.
                        Useful e.g. for randomly selecting from paired end reads.
                        Either a number (can be very big) or a string, from which
                        the first 32 bytes are used.
",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = config::Config::from_args(&args)?;

    // parse seed
    let seed_str = args.opt_str("--seed");
    let seed = seed_str.map(|s| {
        let mut seed = [0; 32];
        if let Ok(num) = s.parse() {
            (&mut seed[..]).write_u64::<BigEndian>(num).unwrap();
        } else {
            (&mut seed[..]).write_all(s.as_bytes()).unwrap();
        }
        seed
    });

    cfg.writer(|writer, vars| {
        if let Some(n_rand) = args.opt_value("--num-seqs")? {
            if let Some(s) = seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_n(&cfg, n_rand, rng, writer, vars)
            } else {
                sample_n(&cfg, n_rand, thread_rng(), writer, vars)
            }
        } else if let Some(p) = args.opt_value::<f32>("--frac")? {
            if !(0f32..=1.).contains(&p) {
                return fail!("Fractions should be between 0 and 1");
            }
            if let Some(s) = seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_prob(&cfg, p, rng, writer, vars)
            } else {
                sample_prob(&cfg, p, thread_rng(), writer, vars)
            }
        } else {
            return fail!("Nothing selected, use either -n or --prob");
        }
    })
}

fn sample_n<R: Rng, W: io::Write>(
    cfg: &config::Config,
    k: usize,
    mut rng: R,
    writer: &mut dyn Writer<W>,
    vars: &mut Vars,
) -> CliResult<()> {
    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    // count

    let mut n = 0;
    cfg.read_simple(|_| {
        n += 1;
        Ok(true)
    })?;

    if n == 0 {
        return Ok(());
    }

    // select randomly
    // TODO: original implementation optimized for small k, becomes very inefficient with large k/n
    // consider reservoir sampling for sufficiently small k
    let mut chosen = BitVec::from_elem(n, false);
    let distr = Uniform::new(0usize, n);
    // TODO: warning if k > n?
    let n_sample = min(k, n);

    for _ in 0..n_sample {
        loop {
            let x = distr.sample(&mut rng);
            if !chosen[x] {
                chosen.set(x, true);
                break;
            }
        }
    }

    // output sequences

    let mut chosen_iter = chosen.into_iter();

    cfg.read(vars, |record, vars| {
        if chosen_iter.next().unwrap() {
            writer.write(&record, vars)?;
        }
        Ok(true)
    })
}

fn sample_prob<R: Rng, W: io::Write>(
    cfg: &config::Config,
    prob: f32,
    mut rng: R,
    writer: &mut dyn Writer<W>,
    vars: &mut Vars,
) -> CliResult<()> {
    assert!((0f32..=1.).contains(&prob));
    // TODO: new_inclusive?
    let distr = Uniform::new(0f32, 1f32);

    cfg.read(vars, |record, vars| {
        if distr.sample(&mut rng) < prob {
            writer.write(&record, vars)?;
        }
        Ok(true)
    })
}
