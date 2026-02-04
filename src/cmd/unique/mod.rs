use std::env::temp_dir;
use std::fmt;
use std::io;
use std::path::PathBuf;

use deepsize::DeepSizeOf;
use rkyv::{Archive, Deserialize, Serialize};
use serde::Serialize as SerdeSerialize;

use crate::CliError;
use crate::cli::Report;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::vec_buf::VecFactory;
use crate::io::IoKind;
use crate::var::varstring::register_var_list;

use super::shared::tmp_store::{Archivable, Key};

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

/// Factor indicating the memory that is found empirically by memory profiling
/// and adjusts the calculated memory usage (based on size of items)
/// to obtain the correct total size, correcting for the extra memory used by
/// the hash map, sorting and other allocations unaccounted for.
static MEM_OVERHEAD: f32 = 1.2;

#[derive(Debug, Clone, Default, SerdeSerialize)]
pub struct UniqueStats {
    pub n_unique: usize,
    pub n_records: u64,
}

impl fmt::Display for UniqueStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} of {} records are unique",
            self.n_unique, self.n_records
        )
    }
}

pub fn run(mut cfg: Config, args: UniqueCommand) -> CliResult<Option<Box<dyn Report>>> {
    let verbose = args.common.general.verbose;
    let quiet = args.common.general.quiet;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    let mut record_buf_factory = VecFactory::new();
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);
    let mut map_writer = args
        .map_out
        .as_ref()
        .map(|path| {
            Ok::<_, CliError>(MapWriter::new(
                cfg.io_writer(IoKind::from_path(path)?)?,
                args.map_fmt,
            ))
        })
        .transpose()?;

    cfg.set_custom_varmodule(Box::<UniqueVars>::default())?;

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let mut varstring_keys = Vec::with_capacity(1);
        cfg.build_vars(|b| register_var_list(&args.key, b, &mut varstring_keys, None, true, true))?;
        let mut key_values = Key::with_size(varstring_keys.len());
        let mut text_buf = vec![Vec::new(); varstring_keys.len()];
        // Depending on the CLI options, different information is needed,
        // which in turn affects how the de-duplicaion is done
        let mut required_info = cfg.with_custom_varmod(|v: &mut UniqueVars| v.required_info());
        let has_placeholders = required_info.is_some();
        if args.map_out.is_some() {
            required_info = Some(RequiredInformation::Ids);
        }

        let mut dedup = Deduplicator::new(
            max_mem,
            has_placeholders,
            args.sort,
            required_info,
            tmp_path,
            args.temp_file_limit,
        );

        let stats = cfg.read(|record, ctx| {
            // assemble key
            key_values.compose_from(&varstring_keys, &mut text_buf, ctx.symbols(), record)?;
            ctx.with_custom_varmod(0, |m: &mut UniqueVars, sym| m.set(&key_values, sym));

            // add record
            dedup.add(
                &key_values,
                || record.id(),
                &mut record_buf_factory,
                |out| format_writer.write(&record, out, ctx),
                io_writer,
                quiet,
            )?;
            Ok(true)
        })?;

        // write unique output (in case of deferred writing)
        // and/or map output
        dedup.write_deferred(io_writer, map_writer.as_mut(), quiet, verbose)?;

        if let Some(writer) = map_writer {
            writer.into_inner().finish()?;
        }
        Ok(Some(
            UniqueStats {
                n_records: stats.n_records,
                n_unique: dedup.n_unique(),
            }
            .to_box(),
        ))
    })
}

#[allow(clippy::doc_overindented_list_items)]
/// Object handling the de-duplication, either in memory or using temporary files.
///
/// There are several modes of operation:
/// 1) Simplest: keep a set of all unique keys in memory and write the unique records
///    immediately to output (in the order in which they occur in the input).
///    In case duplicate IDs should be written to a file later, lists of IDs are
///    collected along with the unique keys.
/// 2) In case the memory limit is reached:
///    a) sort by keys [with associated information] and write records to a temporary
///       file
///    b) start a new in-memory de-duplication process, whereby along each key,
///       the formatted sequence records are kept instead of immediately writing
///       them to the output
///    c) once the memory limit is reached again, write the unique pairs of
///       (key, formatted record) to another temporary file,
///    d) keep going with (b)-(c) until all records are processed
///    e) merge the sorted file batches using a binary heap and at each occurrence of
///       a new unique key write the associated pre-formatted record to the output.
///       Records are unique within each file batch, but duplicates can occur across
///       the different batches, which is why the sorting is necessary.
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
struct Deduplicator {
    inner: DeduplicatorInner,
    tmp_path: Option<PathBuf>,
    file_limit: usize,
}

#[derive(Debug)]
enum DeduplicatorInner {
    Mem(MemDeduplicator),
    File(FileDeduplicator),
}

impl Deduplicator {
    fn new(
        max_mem: usize,
        has_placeholders: bool,
        sort: bool,
        required_info: Option<RequiredInformation>,
        tmp_path: PathBuf,
        file_limit: usize,
    ) -> Self {
        let mem_dedup = MemDeduplicator::new(max_mem, has_placeholders, sort, required_info);
        Self {
            inner: DeduplicatorInner::Mem(mem_dedup),
            tmp_path: Some(tmp_path),
            file_limit,
        }
    }

    // add a key/record and either directly write to output or keep the formatted record for later
    fn add<'a, I, F>(
        &mut self,
        key: &Key,
        id_fn: I,
        vec_factory: &mut VecFactory,
        mut write_fn: F,
        output: &mut dyn io::Write,
        quiet: bool,
    ) -> CliResult<()>
    where
        I: Fn() -> &'a [u8],
        F: FnMut(&mut dyn io::Write) -> CliResult<()>,
    {
        match &mut self.inner {
            DeduplicatorInner::Mem(m) => {
                if !m.add(key, id_fn, vec_factory, &mut write_fn, Some(output))? {
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, writing to temporary file(s). \
                            Consider raising the limit (-M/--max-mem) to speed up de-duplicating. \
                            Use -q/--quiet to silence this message.",
                            m.n_unique()
                        );
                    }
                    let f =
                        m.get_file_sorter(self.tmp_path.take().unwrap(), self.file_limit, quiet)?;
                    self.inner = DeduplicatorInner::File(f);
                }
            }
            DeduplicatorInner::File(f) => {
                f.add(key, id_fn, vec_factory, &mut write_fn, quiet)?;
            }
        }
        Ok(())
    }

    pub fn write_deferred<M: io::Write>(
        &mut self,
        io_writer: &mut dyn io::Write,
        map_writer: Option<&mut MapWriter<M>>,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        match &mut self.inner {
            DeduplicatorInner::Mem(m) => m.write_deferred(io_writer, map_writer),
            DeduplicatorInner::File(f) => f.write_records(io_writer, map_writer, quiet, verbose),
        }
    }

    pub fn n_unique(&self) -> usize {
        match &self.inner {
            DeduplicatorInner::Mem(m) => m.n_unique(),
            DeduplicatorInner::File(f) => f.n_unique(),
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

impl Archivable<'_> for Record {}
