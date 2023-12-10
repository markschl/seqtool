use std::cmp::min;
use std::io::{self, Write};

use bit_vec::BitVec;
use byteorder::{BigEndian, WriteBytesExt};
use clap::Parser;
use rand::{distributions::Uniform, prelude::*};

use crate::config::Config;
use crate::error::CliResult;
use crate::io::output::Writer;
use crate::opt::CommonArgs;
use crate::var::*;

/// Returns a random subset of sequences.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SampleCommand {
    /// Randomly select with probability f returning the given
    /// fraction of sequences on average.
    #[arg(short, long)]
    frac: Option<f32>,

    /// Number of sequences to return
    #[arg(short, long, value_name = "N")]
    num_seqs: Option<usize>,

    /// Use this seed to make the sampling reproducible.
    /// Useful e.g. for randomly selecting from paired end reads.
    /// Either a number (can be very big) or a string, from which
    /// the first 32 bytes are used.
    #[arg(short, long, value_parser = |s: &str| Ok::<_, String>(read_seed(s)))]
    seed: Option<Seed>,

    #[command(flatten)]
    pub common: CommonArgs,
}

type Seed = [u8; 32];

fn read_seed(seed_str: &str) -> Seed {
    let mut seed = [0; 32];
    if let Ok(num) = seed_str.parse() {
        (&mut seed[..]).write_u64::<BigEndian>(num).unwrap();
    } else {
        (&mut seed[..]).write_all(seed_str.as_bytes()).unwrap();
    }
    seed
}

pub fn run(cfg: Config, args: &SampleCommand) -> CliResult<()> {
    cfg.writer(|writer, vars| {
        if let Some(n_rand) = args.num_seqs {
            if let Some(s) = args.seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_n(&cfg, n_rand, rng, writer, vars)
            } else {
                sample_n(&cfg, n_rand, thread_rng(), writer, vars)
            }
        } else if let Some(p) = args.frac {
            if !(0f32..=1.).contains(&p) {
                return fail!("Fractions should be between 0 and 1");
            }
            if let Some(s) = args.seed {
                let rng: StdRng = SeedableRng::from_seed(s);
                sample_prob(&cfg, p, rng, writer, vars)
            } else {
                sample_prob(&cfg, p, thread_rng(), writer, vars)
            }
        } else {
            return fail!("Nothing selected, use either -n/--num-seqs or --frac");
        }
    })
}

fn sample_n<R: Rng, W: io::Write>(
    cfg: &Config,
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
    cfg: &Config,
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
