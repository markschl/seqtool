use std::cmp::min;
use std::io::{self, Write};

use bit_vec::BitVec;
use byteorder::{BigEndian, WriteBytesExt};
use rand::prelude::*;

use cfg;
use error::CliResult;
use io::output::Writer;
use opt;
use var::*;

pub static USAGE: &'static str = concat!(
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
    let cfg = cfg::Config::from_args(&args)?;

    // parse seed
    let seed_str = args.opt_str("--seed");
    let seed = seed_str.map(|s| {
        let mut seed = [0; 32];
        if let Ok(num) = s.parse() {
            (&mut seed[..]).write_u64::<BigEndian>(num).unwrap();
        } else {
            (&mut seed[..]).write(s.as_bytes()).unwrap();
        }
        seed
    });

    cfg.writer(|writer, mut vars| {
        if let Some(n_rand) = args.opt_value("--num-seqs")? {
            if let Some(s) = seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_n(&cfg, n_rand, rng, writer, &mut vars)
            } else {
                sample_n(&cfg, n_rand, thread_rng(), writer, &mut vars)
            }
        } else if let Some(p) = args.opt_value::<f32>("--frac")? {
            if p < 0. || p > 1. {
                return fail!("Fractions should be between 0 and 1");
            }
            if let Some(s) = seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_prob(&cfg, p, rng, writer, &mut vars)
            } else {
                sample_prob(&cfg, p, thread_rng(), writer, &mut vars)
            }
        } else {
            return fail!("Nothing selected, use either -n or --prob");
        }
    })
}

fn sample_n<R: Rng, W: io::Write>(
    cfg: &cfg::Config,
    n_rand: usize,
    mut rng: R,
    writer: &mut Writer<W>,
    mut vars: &mut Vars,
) -> CliResult<()> {
    if cfg.has_stdin() {
        return fail!("Cannot use STDIN as input, since we need to count all sequences before");
    }

    // count

    let mut n = 0;

    cfg.read_sequential(|_| {
        n += 1;
        Ok(true)
    })?;

    if n == 0 {
        return Ok(());
    }

    // select randomly

    let mut chosen = BitVec::from_elem(n, false);

    for _ in 0..min(n_rand, n) {
        loop {
            let x: usize = rng.gen_range(0, n);
            if !chosen[x] {
                chosen.set(x, true);
                break;
            }
        }
    }

    // output sequences

    let mut chosen_iter = chosen.into_iter();

    cfg.read_sequential_var(&mut vars, |record, vars| {
        if chosen_iter.next().unwrap() {
            writer.write(&record, vars)?;
        }
        Ok(true)
    })
}

fn sample_prob<R: Rng, W: io::Write>(
    cfg: &cfg::Config,
    prob: f32,
    mut rng: R,
    writer: &mut Writer<W>,
    mut vars: &mut Vars,
) -> CliResult<()> {
    assert!(prob >= 0. && prob <= 1.);

    cfg.read_sequential_var(&mut vars, |record, vars| {
        if rng.gen::<f32>() < prob {
            writer.write(&record, vars)?;
        }
        Ok(true)
    })
}
