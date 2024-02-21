use std::env::temp_dir;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{value::SimpleValue, vec::VecFactory};
use crate::var::varstring::VarString;

pub mod cli;
pub mod file;
pub mod mem;
pub mod vars;

pub use self::cli::*;
pub use self::file::*;
pub use self::mem::*;
pub use self::vars::*;

/// Factor indicating the memory that is found empirically by memory profiling
/// and adjusts the calculated memory usage (based on size of items)
/// to obtain the correct total size, correcting for the extra memory used by
/// the hash map, Vec::sort() and possibly other allocations (TODO: investigate further)
static MEM_OVERHEAD: f32 = 1.4;

pub fn run(mut cfg: Config, args: &UniqueCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    let mut record_buf_factory = VecFactory::new();
    let mut key_buf = SimpleValue::Text(Vec::new());
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let (var_key, _) = cfg.build_vars(|b| VarString::parse_register(&args.key, b, true))?;
        let count_duplicates = cfg
            .with_command_vars::<UniqueVars, _>(|v, _| Ok(v.unwrap().needs_duplicates()))
            .unwrap();
        let mut dedup = Deduplicator::new(max_mem, count_duplicates);

        cfg.read(|record, ctx| {
            // assemble key
            let key = ctx.command_vars::<UniqueVars, _>(|key_mod, symbols| {
                let key = var_key.get_simple(&mut key_buf, symbols, record, force_numeric)?;
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;
            // add formatted record to hash set (if doensn't exist)
            dedup.insert(
                &key,
                || record_buf_factory.fill_vec(|out| format_writer.write(&record, out, ctx)),
                &tmp_path,
                args.temp_file_limit,
                args.quiet,
            )?;
            Ok(true)
        })?;
        // write unique output
        dedup.write(io_writer, args.sort, args.quiet, verbose)
    })
}

/// Object handling the de-duplication, either in memory or using temporary files
#[derive(Debug)]
enum Deduplicator {
    Mem(MemDeduplicator),
    File(FileDeduplicator),
}

impl Deduplicator {
    fn new(max_mem: usize, count_duplicates: bool) -> Self {
        Self::Mem(MemDeduplicator::new(max_mem, count_duplicates))
    }

    fn insert(
        &mut self,
        key: &SimpleValue,
        record_fn: impl FnMut() -> CliResult<Vec<u8>>,
        tmp_path: &Path,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => {
                if !m.insert(key, record_fn)? {
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, writing to temporary file(s). \
                            Consider raising the limit (-M/--max-mem) to speed up sorting. \
                            Use -q/--quiet to silence this message.",
                            m.len()
                        );
                    }
                    let mut f = m.get_file_sorter(tmp_path.to_owned(), file_limit)?;
                    f.write_to_file(quiet)?;
                    *self = Self::File(f);
                }
            }
            Self::File(f) => {
                f.insert(key, record_fn, quiet)?;
            }
        }
        Ok(())
    }

    fn write(
        &mut self,
        io_writer: &mut dyn Write,
        force_sort: bool,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_records(io_writer, force_sort),
            Self::File(f) => f.write_records(io_writer, quiet, verbose),
        }
    }
}
