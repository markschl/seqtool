use std::env::temp_dir;
use std::io;
use std::path::Path;

use deepsize::DeepSizeOf;
use rkyv::{Archive, Deserialize, Serialize};

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{value::SimpleValue, vec::VecFactory};
use crate::io::output::with_general_io_writer;
use crate::var::varstring::VarString;

pub mod cli;
pub mod file;
pub mod map;
pub mod mem;
pub mod vars;

pub use self::cli::*;
pub use self::file::*;
pub use self::map::*;
pub use self::mem::*;
pub use self::vars::*;

use super::shared::tmp_store::Archivable;

/// Factor indicating the memory that is found empirically by memory profiling
/// and adjusts the calculated memory usage (based on size of items)
/// to obtain the correct total size, correcting for the extra memory used by
/// the hash map, sorting and other allocations unaccounted for.
static MEM_OVERHEAD: f32 = 1.2;

pub fn run(mut cfg: Config, args: &UniqueCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    let mut record_buf_factory = VecFactory::new();
    let mut key_buf = SimpleValue::Text(Box::default());
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);
    let map_out = args.map_out.as_ref();

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let (var_key, _) = cfg.build_vars(|b| VarString::parse_register(&args.key, b, true))?;
        let mut required_info = cfg
            .with_command_vars::<UniqueVars, _>(|v, _| Ok(v.unwrap().required_info()))
            .unwrap();
        let has_placeholders = required_info.is_some();
        if args.map_out.is_some() {
            required_info = Some(RequiredInformation::Ids);
        }

        let mut dedup = Deduplicator::new(max_mem, has_placeholders, args.sort, required_info);

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
            dedup.add(
                &key,
                || record.id(),
                &mut record_buf_factory,
                |out| format_writer.write(&record, out, ctx),
                io_writer,
                &tmp_path,
                args.temp_file_limit,
                args.quiet,
            )?;
            Ok(true)
        })?;

        // write unique output (in case of deferred writing)
        // and/or map output
        if let Some(path) = map_out {
            with_general_io_writer(path, |out| {
                dedup.write_deferred(io_writer, Some((out, args.map_fmt)), args.quiet, verbose)
            })?;
        } else {
            dedup.write_deferred(io_writer, None, args.quiet, verbose)?;
        }
        Ok(())
    })
}

/// Object handling the de-duplication, either in memory or using temporary files.
///
/// There are several modes of operation:
/// 1) Simplest: keep a set of all unique keys in memory and write the unique records
///    immediately to output (in the order in which they occur in the input).
///    In case duplicate IDs should be written to a file later, lists of IDs are
///    collected along with the unique keys.
/// 2) In case the memory limit is reached:
///     a) sort the keys [with associated inforamation] and write them to a temporary
///        file
///     b) start a new in-memory de-duplication process, whereby along each key
///        the formatted sequence records are kept instead of immediately writing
///        them to the output
///     c) once the memory limit is reached again, write the unique pairs of
///        (key, formatted record) to another temporary file,
///     d) keep going with (b)-(c) until all records are processed
///     e) merge the sorted file batches using a binary heap and at each occurrence of
///        a new unique key write the associated pre-formatted record to the output.
///        Records are unique within each file batch, but duplicates can occur across
///        the different batches, which is why the sorting is necessary.
///    In this mode, the order of records is not consistent: initially (before writing
///    to temporary files), unique records are returned as they occur in the input.
///    After the memory limit is reached, the remaining unique records are written
///    sorted by key.
/// 4) In case the output should consistently be sorted by key:
///    Start right away with collecting formatted records along with the unique keys.
///    Only after the input is processed and all keys are finally known,
///    the sorted records are written to the output. Irrespecitive of whether the
///    de-duplication is done in memory or temporary files are used, the behaviour
///    will be the same.
///    *Note*: This mode uses *more memory* and is slower, since we cannot immediately
///    write the unique records when they occur in the input, but always have to
///    store them for later.
/// 5) In case the 'n_duplicate' and/or 'duplicate_ids' variables are used in the
///    output (as header attributes or CSV fields):
///    The formatted records have to be stored during the whole de-duplication process
///    like in (4), and records can only be written to the output once the number
///    and/or list of duplicate IDs is known.
///    However, on-disk sorting is not possible in this mode, and there will be an
///    error if the memory limit is hit.
///    The 'n_duplicate' and 'duplicate_ids' variables are represented by placeholders
///    in the formatted records, which are replaced by their actual values
///    while writing the final output.
///
/// *Summary*: The de-duplication is done in the fastest and most memory-efficient
/// way possible. Hower as a result, the output order is not always the same unless using
/// the `--sort` option.
#[derive(Debug)]
enum Deduplicator {
    Mem(MemDeduplicator),
    File(FileDeduplicator),
}

impl Deduplicator {
    fn new(
        max_mem: usize,
        has_placeholders: bool,
        sort: bool,
        required_info: Option<RequiredInformation>,
    ) -> Self {
        Self::Mem(MemDeduplicator::new(
            max_mem,
            has_placeholders,
            sort,
            required_info,
        ))
    }

    // add a key/record and either directly write to output or keep the formatted record for later
    fn add<'a, I, F>(
        &mut self,
        key: &SimpleValue,
        id_fn: I,
        vec_factory: &mut VecFactory,
        mut write_fn: F,
        output: &mut dyn io::Write,
        tmp_path: &Path,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<()>
    where
        I: Fn() -> &'a [u8],
        F: FnMut(&mut dyn io::Write) -> CliResult<()>,
    {
        match self {
            Self::Mem(m) => {
                if !m.add(key, id_fn, vec_factory, &mut write_fn, Some(output))? {
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, writing to temporary file(s). \
                            Consider raising the limit (-M/--max-mem) to speed up de-duplicating. \
                            Use -q/--quiet to silence this message.",
                            m.len()
                        );
                    }
                    let f = m.get_file_sorter(tmp_path.to_owned(), file_limit, quiet)?;
                    *self = Self::File(f);
                }
            }
            Self::File(f) => {
                f.add(key, id_fn, vec_factory, &mut write_fn, quiet)?;
            }
        }
        Ok(())
    }

    fn write_deferred(
        &mut self,
        io_writer: &mut dyn io::Write,
        map_out: Option<(&mut dyn io::Write, MapFormat)>,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_deferred(io_writer, map_out),
            Self::File(f) => f.write_records(io_writer, map_out, quiet, verbose),
        }
    }
}

/// This struct holds a formatted sequence record and associated information,
/// which is needed by both the `MemDeduplicator` and the `FileDeduplicator`.
#[derive(Archive, Deserialize, Serialize, DeepSizeOf, Debug, Clone, PartialEq)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Record {
    /// The formatted record or `None` if the record has already been written
    /// to the output
    record: Option<Vec<u8>>,
    /// Information about duplicates if needed
    duplicate_info: Option<DuplicateInfo>,
}

impl Record {
    pub fn write_deferred(&self, has_placeholders: bool, out: &mut dyn io::Write) -> CliResult<()> {
        if let Some(text) = self.record.as_deref() {
            // there is a formatted record to write
            // (if `None`, this means that it has already been written)
            if !has_placeholders {
                out.write_all(text)?;
            } else {
                fill_placeholders(text, self.duplicate_info.as_ref().unwrap(), out)?;
            }
        }
        Ok(())
    }
}

impl<'a> Archivable<'a> for Record {}
