use clap::Parser;
use fxhash::FxBuildHasher;
use indexmap::IndexMap;

use crate::config::Config;
use crate::error::CliResult;
use crate::opt::CommonArgs;
use crate::var::varstring::VarString;

use super::sort::{KeyVars, VecFactory};

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

/// De-replicate records, returning only unique ones.
///
/// The order of the records is the same as in the input.
/// Records are de-replicated in memory, so make sure that the unique set
/// will not be too large.
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct UniqueCommand {
    /// The key used to determine, which records are unique.
    /// If not specified, records are de-replicated by the sequence.
    /// The key can be a single variable/function
    /// such as 'id', or a composed string, e.g. '{id}_{desc}'.
    /// For each key, the *first* encountered record is returned, and all
    /// remaining ones with the same key are discarded.
    #[arg(short, long, default_value = "seq")]
    key: String,

    /// Interpret the key as a number instead of text.
    /// This may improve performance if the key is numeric, which could occur with
    /// header attributes or fields from associated lists with metadata.
    #[arg(short, long)]
    numeric: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &UniqueCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let m = Box::new(KeyVars::default());
    cfg.writer_with_custom(Some(m), |writer, io_writer, vars| {
        // assemble key
        let (var_key, _) = vars.build(|b| VarString::var_or_composed(&args.key, b))?;

        // we cannot know the exact length of the input, we just initialize
        // with some capacity
        let mut records = FxIndexMap::with_capacity_and_hasher(1000, Default::default());
        let mut record_buf_factory = VecFactory::new();
        let mut key_buf = Vec::new();

        cfg.read(vars, |record, vars| {
            // initialize with slightly larger capacity than before
            let key = vars.custom_mod::<KeyVars, _>(|key_mod, symbols| {
                let key = var_key
                    .get_dyn(symbols, record, &mut key_buf, force_numeric)?
                    .into();
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;

            if !records.contains_key(&key) {
                let record_out =
                    record_buf_factory.fill_vec(|out| writer.write(&record, out, vars))?;
                records.insert(key, record_out);
            }
            Ok(true)
        })?;

        // then write to output
        for (_, buf) in records {
            io_writer.write_all(&buf)?;
        }
        Ok(())
    })
}
