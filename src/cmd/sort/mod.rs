use std::env::temp_dir;
use std::io::Write;
use std::mem::size_of_val;
use std::path::{Path, PathBuf};

use clap::Parser;
use ordered_float::OrderedFloat;
use rkyv::{Archive, Deserialize, Serialize};

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{bytesize::parse_bytesize, vec::VecFactory};
use crate::opt::CommonArgs;
use crate::var::varstring::{DynValue, VarString};

use self::file::FileSorter;
use self::mem::MemSorter;
use self::var::KeyVars;

pub mod file;
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

    /// Temporary directory to use for sorting (only if memory limit is exceeded)
    #[arg(long)]
    temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, default_value_t = 1000)]
    temp_file_limit: usize,

    /// Silence any warnings
    #[arg(short, long)]
    pub quiet: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

// #[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Hash, Clone, Archive, Serialize, Deserialize)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Archive, Deserialize, Serialize)]
#[archive(compare(PartialEq), check_bytes)]
pub enum Key {
    Text(Vec<u8>),
    Numeric(OrderedFloat<f64>),
    None,
}

impl Key {
    pub fn size(&self) -> usize {
        match self {
            Key::Text(v) => size_of_val(v) + size_of_val(&**v),
            _ => size_of_val(self)
        }
    }
}

impl<'a> From<Option<DynValue<'a>>> for Key {
    fn from(v: Option<DynValue<'a>>) -> Self {
        match v {
            Some(DynValue::Text(v)) => Key::Text(v.to_vec()),
            Some(DynValue::Numeric(v)) => Key::Numeric(OrderedFloat(v)),
            None => Key::None,
        }
    }
}

pub fn run(cfg: Config, args: &SortCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    // if args.max_mem < 1 << 22 {
    //     return fail!("The memory limit should be at least 2MiB");
    // }

    let m = Box::<KeyVars>::default();
    cfg.writer_with_custom(Some(m), |writer, io_writer, vars| {
        // assemble key
        let (var_key, _vtype) = vars.build(|b| VarString::var_or_composed(&args.key, b))?;
        // we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        // let mut records = Vec::with_capacity(10000);
        let mut record_buf_factory = VecFactory::new();
        let mut key_buf = Vec::new();
        let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);
        let mut sorter = Sorter::new(args.reverse, args.max_mem);

        cfg.read(vars, |record, vars| {
            // assemble key
            let key = vars.custom_mod::<KeyVars, _>(|key_mod, symbols| {
                let key = var_key
                    .get_dyn(symbols, record, &mut key_buf, force_numeric)?
                    .into();
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;
            // write formatted record to a buffer
            let record_out = record_buf_factory.fill_vec(|out| writer.write(&record, out, vars))?;
            // add both to the object handing the sorting
            sorter.add(
                Item::new(key, record_out),
                &tmp_path,
                args.temp_file_limit,
                args.quiet,
            )?;
            Ok(true)
        })?;
        // write sorted output
        sorter.write(io_writer, args.temp_file_limit, args.quiet, verbose)
    })
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item {
    pub key: Key,
    pub record: Vec<u8>,
}

impl Item {
    fn new(key: Key, record: Vec<u8>) -> Self {
        Self { key, record }
    }

    fn size(&self) -> usize {
        self.key.size() + size_of_val(&self.record) + size_of_val(&*self.record)
    }
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
                    let mut f = m.get_file_sorter(tmp_path.to_owned())?;
                    f.write_to_file(file_limit, quiet)?;
                    *self = Self::File(f);
                }
            }
            Self::File(f) => {
                f.add(item, file_limit, quiet)?;
            }
        }
        Ok(())
    }

    fn write(
        &mut self,
        io_writer: &mut dyn Write,
        file_limit: usize,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_sorted(io_writer),
            Self::File(f) => f.write_records(io_writer, file_limit, quiet, verbose),
        }
    }
}
