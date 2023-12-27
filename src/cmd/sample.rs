use std::io::{self, Write};
use std::mem::size_of;

use clap::{value_parser, Parser};
use rand::{distributions::Uniform, prelude::*};

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{bytesize::parse_bytesize, vec::VecFactory};
use crate::io::{output::FormatWriter, Record};
use crate::opt::CommonArgs;
use crate::var::*;

/// Returns a random subset of sequences, sorted in input order.
///
/// Either a fixed number (-n/--num-seqs) or an approximate fraction (-p/--prob)
/// can be sampled (see help for these options). The records are returned in
/// input order.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SampleCommand {
    /// Randomly select a fixed number of sequences.
    /// In case speed is important, consider -p/--prob.
    /// For lower memory usage (but less speed), supply -2/--to-pass.
    #[arg(short, long, value_name = "N", value_parser = value_parser!(u64).range(1..))]
    num_seqs: Option<u64>,

    /// Instead of a fixed number, include each sequence with the given probability.
    /// There is no guarantee about an exact number of returned sequences, but
    /// the fraction of returned sequences will be near the specified probability.
    #[arg(short, long)]
    prob: Option<f32>,

    /// Use a seed to make the sampling reproducible.
    /// Useful e.g. for randomly selecting from paired end reads.
    /// Either a number (can be very large) or an ASCII string, from which
    /// the first 32 characters are used.
    #[arg(short, long, value_parser = |s: &str| Ok::<_, String>(read_seed(s)))]
    seed: Option<Seed>,

    /// Use two-pass sampling with -n/--num-seqs:
    /// (1) read all files to obtain the total number of sequences,
    /// (2) read again, and return the selected sequences.
    /// This uses less memory, but does not work with STDIN and may be especially
    /// slow with compressed files. Automatically activated if the -M/--max-mem
    /// limit is reached.
    #[arg(short = '2', long)]
    two_pass: bool,

    /// Maximum amount of memory to use for sequences.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    /// This limit may be hit if a large number of sequences is chosen
    /// (-n/--num-seqs). If reading from a file (not STDIN), the program will
    /// automatically switch to two-pass sampling mode.
    /// Alternatively, conider using -p/--prob if the number of returned sequences
    /// does not have to be exact.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    max_mem: usize,

    /// Silence any warnings
    #[arg(short, long)]
    pub quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Clone, Debug)]
enum Seed {
    Number(u64),
    Array([u8; 32]),
}

fn read_seed(seed_str: &str) -> Seed {
    if let Ok(num) = seed_str.parse() {
        Seed::Number(num)
    } else {
        let mut seed = [0; 32];
        (&mut seed[..]).write_all(seed_str.as_bytes()).unwrap();
        Seed::Array(seed)
    }
}

// This is a fast non-cryptographic RNG optimized for 64-bit.
// We use it for all platforms to ensure reproducibility.
// TODO: examine performance on 32-bit platforms
pub type DefaultRng = rand_xoshiro::Xoshiro256PlusPlus;

pub fn run(cfg: Config, args: &SampleCommand) -> CliResult<()> {
    cfg.writer(|writer, io_writer, vars| {
        let rng = match args.seed {
            Some(Seed::Number(s)) => DefaultRng::seed_from_u64(s),
            Some(Seed::Array(s)) => DefaultRng::from_seed(s),
            None => DefaultRng::from_entropy(),
        };
        if let Some(amount) = args.num_seqs {
            let amount = amount as usize;
            sample_n(
                &cfg,
                amount,
                rng,
                writer,
                io_writer,
                vars,
                args.max_mem,
                args.two_pass,
                args.quiet,
            )
        } else if let Some(p) = args.prob {
            sample_prob(&cfg, p, rng, writer, io_writer, vars)
        } else {
            fail!("Nothing selected, use either -n/--num-seqs or -p/--prob")
        }
    })
}

fn sample_n<R: Rng + Clone>(
    cfg: &Config,
    amount: usize,
    rng: R,
    writer: &mut dyn FormatWriter,
    io_writer: &mut dyn io::Write,
    vars: &mut Vars,
    max_mem: usize,
    two_pass: bool,
    quiet: bool,
) -> CliResult<()> {
    let mut sampler = ReservoirSampler::new(amount, rng, two_pass, max_mem)?;
    cfg.read_simple(|record| {
        sampler.sample(record, writer, vars, quiet)?;
        Ok(true)
    })?;
    sampler.write(cfg, writer, vars, io_writer)
}

// Ensures consistency between 32 and 64-bit platforms,
// copied without modification from rand crate
#[inline]
fn gen_index<R: Rng + ?Sized>(rng: &mut R, ubound: usize) -> usize {
    if ubound <= (core::u32::MAX as usize) {
        rng.gen_range(0..ubound as u32) as usize
    } else {
        rng.gen_range(0..ubound)
    }
}

enum ReservoirSampler<R: Rng + Clone> {
    Records(RecordsSampler<R>),
    Indices(IndexSampler<R>),
}

impl<R: Rng + Clone> ReservoirSampler<R> {
    fn new(amount: usize, rng: R, two_pass: bool, max_mem: usize) -> Result<Self, String> {
        if two_pass {
            Ok(ReservoirSampler::Indices(IndexSampler::new(
                amount, rng, max_mem, None,
            )?))
        } else {
            Ok(ReservoirSampler::Records(RecordsSampler::new(
                amount, rng, max_mem,
            )))
        }
    }

    fn sample(
        &mut self,
        record: &dyn Record,
        writer: &mut dyn FormatWriter,
        vars: &mut Vars,
        quiet: bool,
    ) -> CliResult<()> {
        match self {
            ReservoirSampler::Records(ref mut s) => {
                if !s.sample(record, writer, vars)? {
                    let s = s.get_index_sampler()?;
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, switching to two-pass sampling. \
                            Consider raising the limit (-M/--max-mem) or activating two-pass sampling \
                            from the start (-2/--two-pass). Use -q/--quiet to silence this message.",
                            s.len()
                        );
                    }
                    *self = ReservoirSampler::Indices(s);
                }
                Ok(())
            }
            ReservoirSampler::Indices(ref mut s) => s.sample(),
        }
    }

    fn write(
        self,
        cfg: &Config,
        writer: &mut dyn FormatWriter,
        vars: &mut Vars,
        io_writer: &mut dyn io::Write,
    ) -> CliResult<()> {
        match self {
            ReservoirSampler::Records(s) => s.write(io_writer),
            ReservoirSampler::Indices(s) => s.write(cfg, vars, writer, io_writer),
        }
    }
}

/// Handles sampling of a fixed number of records without counting them beforehand.
/// This should be the best strategy if the number of records to be selected is
/// much smaller than the total number of records in the input, and
/// it fits easily into the buffer.
struct RecordsSampler<R: Rng + Clone> {
    rng: R,
    amount: usize,
    reservoir: Vec<(usize, Vec<u8>)>,
    vec_factory: VecFactory,
    i: usize,
    mem: usize,
    max_mem: usize,
}

impl<R: Rng + Clone> RecordsSampler<R> {
    fn new(amount: usize, rng: R, max_mem: usize) -> Self {
        Self {
            rng,
            amount,
            reservoir: Vec::with_capacity(amount),
            vec_factory: VecFactory::new(),
            i: 0,
            mem: 0,
            max_mem,
        }
    }

    fn sample(
        &mut self,
        record: &dyn Record,
        writer: &mut dyn FormatWriter,
        vars: &mut Vars,
    ) -> CliResult<bool> {
        // simple reservoir sampling
        // The code very similar to rand::seq::choose_multiple_fill or choose_multiple,
        // (in general, implements the "algorithm R" here:
        // https://en.wikipedia.org/wiki/Reservoir_sampling#Simple:_Algorithm_R).
        // Initially, this requires a lot of copying, but with large collections,
        // copying becomes less and less frequent.
        // Writes data into formatted text, whose allocations are reused when
        // replacing.
        // Returns false if the memory limit is exceeded
        // The actual memory usage can actually be larger than the limit, since
        // the first record exceeding the limit still has to be handled.
        if self.i < self.amount {
            let fmt_rec = self
                .vec_factory
                .fill_vec(|out| writer.write(&record, out, vars))?;
            self.mem += size_of::<usize>() + fmt_rec.len();
            self.reservoir.push((self.i, fmt_rec));
            if self.mem >= self.max_mem {
                self.i += 1;
                return Ok(false);
            }
        } else {
            let k = gen_index(&mut self.rng, self.i + 1);
            if let Some((idx, fmt_rec)) = self.reservoir.get_mut(k) {
                self.mem -= fmt_rec.len();
                *idx = self.i;
                fmt_rec.clear();
                writer.write(&record, fmt_rec, vars)?;
                self.mem += fmt_rec.len();
                if self.mem >= self.max_mem {
                    self.i += 1;
                    return Ok(false);
                }
            }
        }
        self.i += 1;
        Ok(true)
    }

    fn write(mut self, io_writer: &mut dyn Write) -> CliResult<()> {
        // Sort by index to be consistent with IndexSampler.
        // This should not take too long compared to the other steps.
        self.reservoir.sort_by_key(|(i, _)| *i);

        // write the contents of the reservoir
        for (_, rec) in self.reservoir {
            io_writer.write_all(&rec)?;
        }
        Ok(())
    }

    fn get_index_sampler(&mut self) -> Result<IndexSampler<R>, String> {
        let idx = self.reservoir.iter().map(|(i, _)| *i).collect();
        IndexSampler::new(
            self.amount,
            self.rng.clone(),
            self.max_mem,
            Some((self.i, idx)),
        )
    }
}

/// Handles sampling of a fixed number of indices from the input.
/// In a second pass, the `write` function reads the input again,
/// and the records belonging to the chosen indices are written.
struct IndexSampler<R: Rng> {
    rng: R,
    amount: usize,
    reservoir: Vec<usize>,
    i: usize,
}

impl<R: Rng> IndexSampler<R> {
    /// pre_sampled: allows continuing an already started sampling.
    fn new(
        amount: usize,
        rng: R,
        max_mem: usize,
        pre_sampled: Option<(usize, Vec<usize>)>,
    ) -> Result<Self, String> {
        if amount * size_of::<usize>() > max_mem {
            return Err(format!(
                "Not enough memory to sample {} records. \
                Consider raising the memory limit (-M/--max-mem) or using -p/--prob.",
                amount
            ));
        }
        let (i, reservoir) = pre_sampled.unwrap_or((0, Vec::with_capacity(amount)));
        Ok(Self {
            rng,
            amount,
            reservoir,
            i,
        })
    }

    fn sample(&mut self) -> CliResult<()> {
        if self.i < self.amount {
            self.reservoir.push(self.i);
        } else {
            let k = gen_index(&mut self.rng, self.i + 1);
            if let Some(slot) = self.reservoir.get_mut(k) {
                *slot = self.i;
            }
        }
        self.i += 1;
        Ok(())
    }

    fn write(
        mut self,
        cfg: &Config,
        vars: &mut Vars,
        writer: &mut dyn FormatWriter,
        io_writer: &mut dyn Write,
    ) -> CliResult<()> {
        // Next, read again and write the selected records.
        // In order to do that, we pre-sort the indices.
        self.reservoir.sort();

        let mut chosen_iter = self.reservoir.into_iter();
        let mut next_index = chosen_iter.next().unwrap();
        let mut i = 0;
        cfg.read(vars, |record, vars| {
            if i == next_index {
                writer.write(&record, io_writer, vars)?;
                next_index = match chosen_iter.next() {
                    Some(i) => i,
                    // done, we can stop parsing
                    None => return Ok(false),
                };
            }
            i += 1;
            Ok(true)
        })
    }

    fn len(&self) -> usize {
        self.reservoir.len()
    }
}

fn sample_prob<R: Rng>(
    cfg: &Config,
    prob: f32,
    mut rng: R,
    writer: &mut dyn FormatWriter,
    io_writer: &mut dyn io::Write,
    vars: &mut Vars,
) -> CliResult<()> {
    if !(0f32..1.).contains(&prob) {
        return fail!("Fractions should be between 0 and 1 (but still < 1)");
    }
    let distr = Uniform::new(0f32, 1f32);

    cfg.read(vars, |record, vars| {
        if distr.sample(&mut rng) < prob {
            writer.write(&record, io_writer, vars)?;
        }
        Ok(true)
    })
}
