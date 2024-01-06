use std::env::temp_dir;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{bytesize::parse_bytesize, value::SimpleValue, vec::VecFactory};
use crate::var::varstring::VarString;

use self::file::FileSorter;
use self::item::Item;
use self::mem::MemSorter;
use self::var::KeyVars;

pub mod file;
pub mod item;
pub mod mem;
pub mod var;

/// Sort records by sequence or any other criterion.
///
/// Records are sorted in memory, it is up to the user of this function
/// to ensure that the whole input will fit into memory.
/// The default sort is by sequence.
///
/// The -k/--key option allows sorting by any variable/function, expression, or
/// text composed of them (see --key help).
///
/// The actual value of the key is available through the 'key' variable. It can
/// be written to a header attribute or TSV field.
/// This may be useful with JavaScript expressions, whose evaluation takes time,
/// and whose result should be written to the headers, e.g.:
/// 'st sort -nk '{{ id.substring(3, 5) }}' -a id_num='{key}' input.fasta'
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SortCommand {
    /// The key used to sort the records. If not specified, records are
    /// sorted by the sequence.
    /// The key can be a single variable/function
    /// such as 'id', or a composed string, e.g. '{id}_{desc}'.
    /// To sort by a FASTA/FASTQ attribute in the form '>id;size=123', specify
    /// --key 'attr(size)' --numeric.
    /// Regarding formulas returning mixed text/numbers, the sorted records with
    /// text keys will be returned first and the sorted number records after them.
    /// Furthermore, NaN and missing values (null/undefined in JS expressions,
    /// missing `opt_attr()` values or missing entries in associated metadata)
    /// will appear last.
    #[arg(short, long, default_value = "seq")]
    key: String,

    /// Interpret the key as a number instead of text.
    /// If not specified, the variable type determines, whether the key
    /// is numeric or not.
    /// However, header attributes or fields from associated lists with metadata
    /// may also need to be interpreted as a number, which can be done by
    /// specifying --numeric.
    #[arg(short, long)]
    numeric: bool,

    /// Sort in reverse order
    #[arg(short, long)]
    reverse: bool,

    /// Maximum amount of memory to use for sorting.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    max_mem: usize,

    /// Path to temporary directory (only if memory limit is exceeded)
    #[arg(long)]
    temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, default_value_t = 1000)]
    temp_file_limit: usize,

    /// Silence any warnings
    #[arg(short, long)]
    quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

/// Factor indicating the memory that is found empirically by memory profiling
/// and adjusts the calculated memory usage (based on size of items)
/// to obtain the correct total size, correcting for the extra memory used by
/// Vec::sort() and other allocations.
static MEM_OVERHEAD: f32 = 1.25;

pub fn run(mut cfg: Config, args: &SortCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    // TODO: not activated, since we use a low limit for testing
    // if args.max_mem < 1 << 22 {
    //     return fail!("The memory limit should be at least 2MiB");
    // }
    let mut record_buf_factory = VecFactory::new();
    let mut key_buf = SimpleValue::Text(Vec::new());
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);
    let mut sorter = Sorter::new(args.reverse, max_mem);

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let (var_key, _vtype) = cfg.build_vars(|b| VarString::var_or_composed(&args.key, b))?;

        cfg.read(|record, ctx| {
            // assemble key
            let key = ctx.command_vars::<KeyVars, _>(|key_mod, symbols| {
                let key = var_key.get_simple(&mut key_buf, symbols, record, force_numeric)?;
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;
            // write formatted record to a buffer
            let record_out =
                record_buf_factory.fill_vec(|out| format_writer.write(&record, out, ctx))?;
            // add both to the object handing the sorting
            sorter.add(
                Item::new(key.into_owned(), record_out),
                &tmp_path,
                args.temp_file_limit,
                args.quiet,
            )?;
            Ok(true)
        })?;
        // write sorted output
        sorter.write(io_writer, args.quiet, verbose)
    })
}

#[derive(Debug)]
enum Sorter {
    Mem(MemSorter),
    File(FileSorter),
}

impl Sorter {
    fn new(reverse: bool, max_mem: usize) -> Self {
        Self::Mem(MemSorter::new(reverse, max_mem))
    }

    fn add(
        &mut self,
        item: Item,
        tmp_path: &Path,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => {
                if !m.add(item) {
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
                f.add(item, quiet)?;
            }
        }
        Ok(())
    }

    fn write(&mut self, io_writer: &mut dyn Write, quiet: bool, verbose: bool) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_sorted(io_writer),
            Self::File(f) => f.write_records(io_writer, quiet, verbose),
        }
    }
}
