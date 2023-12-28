use std::cmp::{max, min, Ordering, Reverse};
use std::collections::BinaryHeap;
use std::env::temp_dir;
use std::fs::{File, remove_file};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::mem::size_of_val;
use std::path::{Path, PathBuf};

use byteorder::{ReadBytesExt, LE};
use clap::Parser;
use ordered_float::OrderedFloat;
use rkyv::{
    ser::{
        serializers::{AlignedSerializer, BufferScratch, CompositeSerializer},
        Serializer,
    },
    AlignedVec, Archive, Deserialize, Infallible, Serialize,
};
use tempdir::TempDir;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{bytesize::parse_bytesize, vec::VecFactory};
use crate::opt::CommonArgs;
use crate::var::{
    symbols::{SymbolTable, VarType},
    varstring::{DynValue, VarString},
    Func, VarBuilder, VarHelp, VarProvider,
};

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

impl<'a> From<Option<DynValue<'a>>> for Key {
    fn from(v: Option<DynValue<'a>>) -> Self {
        match v {
            Some(DynValue::Text(v)) => Key::Text(v.to_vec()),
            Some(DynValue::Numeric(v)) => Key::Numeric(OrderedFloat(v)),
            None => Key::None,
        }
    }
}

/// Warning limit for number of temporary files
const TEMP_FILE_WARN_LIMIT: usize = 50;

pub fn run(cfg: Config, args: &SortCommand) -> CliResult<()> {
    let force_numeric = args.numeric;
    let verbose = args.common.general.verbose;
    // if args.max_mem < 1 << 22 {
    //     return fail!("The memory limit should be at least 2MiB");
    // }

    let m = Box::new(KeyVars::default());
    cfg.writer_with_custom(Some(m), |writer, io_writer, vars| {
        // assemble key
        let (var_key, _vtype) = vars.build(|b| VarString::var_or_composed(&args.key, b))?;
        // we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        // let mut records = Vec::with_capacity(10000);
        let mut record_buf_factory = VecFactory::new();
        let mut key_buf = Vec::new();
        let tmp_path = args.temp_dir.clone().unwrap_or_else(|| temp_dir());
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
struct Item {
    pub key: Key,
    pub record: Vec<u8>,
}

impl Item {
    fn new(key: Key, record: Vec<u8>) -> Self {
        Self { key, record }
    }
}

#[derive(Debug, Clone)]
struct MemSorter {
    records: Vec<Item>,
    reverse: bool,
    mem: usize,
    max_mem: usize,
}

impl MemSorter {
    fn new(reverse: bool, max_mem: usize) -> Self {
        Self {
            // we cannot know the exact length of the input, we just initialize
            // with capacity that should at least hold some records, while still
            // not using too much memory
            records: Vec::with_capacity(max(1, min(10000, max_mem / 400))),
            reverse,
            mem: 0,
            max_mem,
        }
    }

    fn add(&mut self, item: Item) -> bool {
        self.mem += size_of_val(&item);
        self.records.push(item);
        self.mem < self.max_mem
    }

    fn sort(&mut self) {
        if !self.reverse {
            self.records.sort_by(|i1, i2| i1.key.cmp(&i2.key));
        } else {
            self.records.sort_by(|i1, i2| i2.key.cmp(&i1.key));
        }
    }

    fn clear(&mut self) {
        self.records.clear();
        self.mem = 0;
    }

    fn len(&self) -> usize {
        self.records.len()
    }

    fn write_sorted(&mut self, io_writer: &mut dyn Write) -> CliResult<()> {
        self.sort();
        for item in &self.records {
            io_writer.write_all(&item.record)?;
        }
        Ok(())
    }

    fn get_file_sorter(&mut self, tmp_dir: PathBuf) -> io::Result<FileSorter> {
        let mut other = MemSorter::new(self.reverse, self.max_mem);
        other.records = self.records.drain(..).collect();
        FileSorter::from_mem(other, tmp_dir)
    }

    fn serialize_sorted(&mut self, mut io_writer: impl Write) -> CliResult<usize> {
        self.sort();
        let mut out = AlignedVec::new();
        let mut scratch = AlignedVec::new();
        for item in &self.records {
            out.clear();
            let mut serializer = CompositeSerializer::new(
                AlignedSerializer::new(&mut out),
                BufferScratch::new(&mut scratch),
                Infallible,
            );
            serializer.serialize_value(item).unwrap();
            let buf = serializer.into_components().0.into_inner();
            io_writer.write_all(&buf.len().to_le_bytes())?;
            io_writer.write_all(buf)?;
        }
        Ok(self.records.len())
    }

    fn deserialize_item(mut io_reader: impl Read, buf: &mut Vec<u8>) -> CliResult<Option<Item>> {
        let len = match io_reader.read_u64::<LE>() {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            res @ _ => res?,
        };
        buf.clear();
        buf.resize(len as usize, 0);
        io_reader.read_exact(buf)?;
        let archived = rkyv::check_archived_root::<Item>(&buf[..]).unwrap();
        // TODO: unsafe appears to save ~ 25% of time, add feature for activating unsafe?
        // let archived = unsafe { rkyv::archived_root::<Item>(&buf[..]) };
        let item = archived.deserialize(&mut Infallible).unwrap();
        Ok(Some(item))
    }
}

/// Wrapper type for items, which are sorted by key only
#[derive(Debug, Clone)]
struct ItemOrd {
    item: Item,
    reverse: bool,
    source: usize,
}

impl ItemOrd {
    fn new(item: Item, reverse: bool, source: usize) -> Self {
        Self {
            item,
            reverse,
            source,
        }
    }
}

impl PartialOrd for ItemOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if !self.reverse {
            self.item.key.partial_cmp(&other.item.key)
        } else {
            other.item.key.partial_cmp(&self.item.key)
        }
    }
}

impl PartialEq for ItemOrd {
    fn eq(&self, other: &Self) -> bool {
        self.item.key == other.item.key
    }
}

impl Eq for ItemOrd {}

impl Ord for ItemOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        if !self.reverse {
            self.item.key.cmp(&other.item.key)
        } else {
            other.item.key.cmp(&self.item.key)
        }
    }
}

#[derive(Debug)]
struct FileSorter {
    mem_sorter: MemSorter,
    files: Vec<PathBuf>,
    tmp_dir: TempDir,
    n_written: usize,
}

impl FileSorter {
    fn from_mem(mem_sorter: MemSorter, tmp_dir: PathBuf) -> io::Result<Self> {
        Ok(Self {
            mem_sorter,
            files: Vec::new(),
            tmp_dir: TempDir::new_in(&tmp_dir, "st_sort_")?,
            n_written: 0,
        })
    }

    fn add(&mut self, item: Item, file_limit: usize, quiet: bool) -> CliResult<bool> {
        if !self.mem_sorter.add(item) {
            self.to_file(file_limit, quiet)?;
        }
        Ok(true)
    }

    fn to_file(&mut self, file_limit: usize, quiet: bool) -> CliResult<()> {
        if self.mem_sorter.len() > 0 && !quiet {
            if self.files.len() == TEMP_FILE_WARN_LIMIT {
                eprintln!(
                    "Warning: sequence sorting resulted in many temporary files ({}). \
                    Consider increasing the memory limit (-M/--max-mem). \
                    Supply -q/--quiet to silence this warning.",
                    TEMP_FILE_WARN_LIMIT
                )
            }
            if self.files.len() == file_limit {
                return fail!(
                    "Too many temporary files ({}) created by sort command. \
                    Try a higher memory limit (-M/--max-mem)",
                    file_limit
                );
            }
            let new_path = self
                .tmp_dir
                .path()
                .join(format!("st_sort_{}.tmp", self.files.len()));
            let mut bufwriter = BufWriter::new(File::create(&new_path)?);
            self.n_written += self.mem_sorter.serialize_sorted(&mut bufwriter)?;
            bufwriter.get_mut().sync_all()?;
            self.mem_sorter.clear();
            self.files.push(new_path);
        }
        Ok(())
    }

    fn write_records(
        &mut self,
        io_writer: &mut dyn Write,
        file_limit: usize,
        quiet: bool,
        verbose: bool,
    ) -> CliResult<()> {
        // write last chunk of records
        self.to_file(file_limit, quiet)?;

        if verbose {
            eprintln!(
                "Sorted {} records using {} temporary files ({:.1}) records per file on average.",
                self.n_written,
                self.files.len(),
                self.n_written as f64 / self.files.len() as f64
            );
        }

        {
            // readers for all sorted file chunks
            let mut readers = self
                .files
                .iter_mut()
                .map(|path| Ok(BufReader::new(File::open(path)?)))
                // .map(|path| {
                //     let bufreader = BufReader::new(File::open(path)?);
                //     Ok(lz4::Decoder::new(bufreader)?)
                // })
                .collect::<CliResult<Vec<_>>>()?;

            // use k-way merging of sorted chunks with a min-heap to obtain
            // the final sorted output
            let mut buf = Vec::new();
            let mut heap = BinaryHeap::with_capacity(self.files.len());
            for (i, rdr) in readers.iter_mut().enumerate() {
                if let Some(item) = MemSorter::deserialize_item(rdr, &mut buf)? {
                    heap.push(Reverse(ItemOrd::new(item, self.mem_sorter.reverse, i)));
                }
            }
            while let Some(top) = heap.pop() {
                if let Some(next_item) =
                    MemSorter::deserialize_item(&mut readers[top.0.source], &mut buf)?
                {
                    heap.push(Reverse(ItemOrd::new(
                        next_item,
                        self.mem_sorter.reverse,
                        top.0.source,
                    )));
                }
                io_writer.write_all(&top.0.item.record)?;
            }
        }
        // clean up
        for path in self.files.drain(..) {
            remove_file(path)?;
        }
        Ok(())
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
                    f.to_file(file_limit, quiet)?;
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

#[derive(Debug)]
pub struct KeyVarHelp;

impl VarHelp for KeyVarHelp {
    fn name(&self) -> &'static str {
        "Sort command variables"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "key",
            "The value of the key (-k/--key argument). \
            The default key is the sequence.",
        )])
    }
}

#[derive(Debug, Default)]
pub struct KeyVars {
    id: Option<usize>,
}

impl KeyVars {
    pub fn set(&mut self, key: &Key, symbols: &mut SymbolTable) {
        self.id.map(|var_id| {
            let v = symbols.get_mut(var_id);
            match key {
                Key::Text(t) => v.inner_mut().set_text(t),
                Key::Numeric(n) => v.inner_mut().set_float(n.0),
                Key::None => v.set_none(),
            }
        });
    }
}

impl VarProvider for KeyVars {
    fn help(&self) -> &dyn VarHelp {
        &KeyVarHelp
    }

    fn allow_dependent(&self) -> bool {
        false
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<Option<VarType>>> {
        if var.name == "key" {
            var.ensure_no_args()?;
            self.id = Some(b.symbol_id());
            return Ok(Some(None));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.id.is_some()
    }
}
