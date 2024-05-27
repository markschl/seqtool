use std::collections::hash_map::Entry;
use std::fmt;
use std::io;
use std::str::FromStr;

use csv::{self, ByteRecord, Reader, ReaderBuilder};

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::helpers::{DefaultHashMap as HashMap, DefaultHashSet as HashSet};
use crate::io::{
    input::{get_io_reader, InputKind},
    FileInfo, FormatVariant, QualConverter, Record,
};
use crate::var::{attr::Attributes, parser::Arg, symbols::SymbolTable, VarBuilder, VarStore};
use crate::CliResult;

use super::VarProvider;

variable_enum! {
    /// # Access metadata from delimited text files
    ///
    /// The following functions allow accessing associated metadata from
    /// plain delimited text files (optionally compressed, extension auto-recognized).
    ///
    /// Metadata files must always contain a column with the sequence ID
    /// (default: 1st column; change with `--meta-idcol`).
    ///
    /// The column delimiter is guessed from the extension or can be specified
    /// with `--meta-delim`.
    /// `.csv` is interpreted as comma(,)-delimited, `.tsv`/`.txt` or other (unknown)
    /// extensions are assumed to be tab-delimited.
    ///
    /// The first line is implicitly assumed to contain
    /// column names if a non-numeric field name is requested, e.g. `meta(fieldname)`.
    /// Use `--meta-header` to explicitly enable header lines even if column names
    /// are all numeric.
    ///
    /// Multiple metadata files can be supplied (`-m file1 -m file2 -m file3 ...`) and
    /// are addressed via `file-num` (see function descriptions).
    /// For maximum performance, provide metadata records in the same order as
    /// sequence records.
    ///
    /// *Note:* Specify `--dup-ids` if the sequence input is expected to contain
    /// duplicate IDs (which is rather unusual). See the help page (`-h/--help`)
    /// for more information.
    ///
    ///
    /// # Examples
    ///
    /// Add taxonomic lineages to the FASTA headers (after a space).
    /// The taxonomy is stored in a GZIP-compressed TSV file (column no. 2)
    /// to the FASTA headers
    ///
    /// `st set -m taxonomy.tsv.gz -d '{meta(2)}' input.fa > output.fa`
    ///
    /// >id1 k__Fungi,p__Ascomycota,c__Sordariomycetes,(...),s__Trichoderma_atroviride
    /// SEQUENCE
    /// >id2 k__Fungi,p__Ascomycota,c__Eurotiomycetes,(...),s__Penicillium_aurantiocandidum
    /// SEQUENCE
    /// (...)
    ///
    ///
    /// Add metadata from an Excel-generated CSV file (semicolon delimiter)
    /// to sequence headers as attributes (`-a/--attr`)
    ///
    /// `st pass -m metadata.csv --meta-sep ';' -a 'info={meta("column name")}' input.fa > output.fa`
    ///
    /// >id1 info=some_value
    /// SEQUENCE
    /// >id2 info=other_value
    /// SEQUENCE
    /// (...)
    ///
    ///
    /// Extract subsequences given a set of coordinates stored in a BED file
    /// (equivalent to `bedtools getfasta`)
    ///
    /// `st trim -m coordinates.bed -0 {meta(2)}..{meta(3)} input.fa > output.fa`
    ///
    ///
    /// Filter sequences by ID, retaining only those present in the given text file
    ///
    /// `st filter -m selected_ids.txt 'has_meta()' input.fa > output.fa`
    MetaVar {
        /// Obtain a value an associated delimited text file supplied with `-m` or `--meta`.
        /// Individual columns from entries with matching record IDs are selected by number
        /// (1, 2, 3, etc.) or by their name according to the column names in the first row.
        /// Missing entries are not allowed.
        /// Column names can be in 'single' or "double" quotes (but quoting is only required
        /// in Javascript expressions).
        ///
        /// If there are multiple metadata files supplied with -m/--meta
        /// (`-m file1 -m file2 -m file3, ...`), the specific file can be referenced
        /// by supplying `<file-number>` (1, 2, 3, ...) as first argument, followed by
        /// the column number or name. This is not necessary if only a single file is supplied.
        Meta(Text) { column: String, file_number: usize = 1 },
        /// Like `meta(...)`, but metadata entries can be missing, i.e. not every sequence
        /// record ID needs a matching metadata entry.
        /// Missing values will result in 'N/A' if written to the output, or 'undefined'
        /// in JavaScript expressions.
        OptMeta(Text) { column: String, file_number: usize = 1 },
        /// Returns `true` if the given record has a metadata entry with the same ID in
        /// the in the given file. In case of multiple files, the file number
        /// must be supplied as an argument.
        HasMeta(Boolean) { file_number: usize = 1 },
    }
}

#[derive(Debug, Clone, PartialEq)]
enum MetaVarKind {
    // column index, allow missing
    Col(usize, bool),
    Exists,
}

#[derive(Debug)]
pub struct MetaVars {
    // metadata readers together with registered variabbles (var_id, col_idx)
    readers: Vec<(MetaReader, VarStore<MetaVarKind>)>,
}

impl MetaVars {
    pub fn new<I: IntoIterator<Item = P>, P: AsRef<str>>(
        paths: I,
        delim: Option<u8>,
        dup_ids: bool,
    ) -> CliResult<Self> {
        let readers = paths
            .into_iter()
            .map(|path| {
                Ok((
                    MetaReader::new(path.as_ref(), delim, dup_ids)?,
                    VarStore::default(),
                ))
            })
            .collect::<CliResult<_>>()?;
        Ok(Self { readers })
    }

    pub fn set_id_col(mut self, id_col: u32) -> Self {
        for (rdr, _) in &mut self.readers {
            rdr.set_id_col(id_col);
        }
        self
    }

    pub fn set_has_header(mut self, has_header: bool) -> Self {
        for (rdr, _) in &mut self.readers {
            rdr.set_has_header(has_header);
        }
        self
    }

    fn mut_reader(
        &mut self,
        file_num: usize,
        func_name: &str,
    ) -> Result<(&mut MetaReader, &mut VarStore<MetaVarKind>), String> {
        if file_num == 0 {
            return fail!("Invalid metadata file no. requested: 0",);
        }
        // there should be at least one metadata file
        if self.readers.is_empty() {
            return fail!("The '{}' function is used, but no metadata source was supplied with -m/--meta <file>.", func_name);
        }
        let n_readers = self.readers.len();
        self.readers
            .get_mut(file_num - 1)
            .map(|(rdr, vars)| (rdr, vars))
            .ok_or_else(|| {
                format!(
                    "Metadata file no. {} was requested by `{}`, \
                    but only {} metadata source(s) were supplied with -m/--meta",
                    file_num, func_name, n_readers
                )
            })
    }
}

impl VarProvider for MetaVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(MetaVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        if let Some((var, out_type)) = MetaVar::from_func(name, args)? {
            use MetaVar::*;
            let (kind, vars) = match var {
                Meta {
                    ref column,
                    file_number,
                } => {
                    let (rdr, vars) = self.mut_reader(file_number, "meta")?;
                    (MetaVarKind::Col(rdr.get_col_index(column)?, false), vars)
                }
                OptMeta {
                    ref column,
                    file_number,
                } => {
                    let (rdr, vars) = self.mut_reader(file_number, "opt_meta")?;
                    (MetaVarKind::Col(rdr.get_col_index(column)?, true), vars)
                }
                HasMeta { file_number } => {
                    let (_, vars) = self.mut_reader(file_number, "has_meta")?;
                    (MetaVarKind::Exists, vars)
                }
            };
            let symbol_id = builder.store_register(kind, vars);
            return Ok(Some((symbol_id, out_type)));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        self.readers.iter().any(|(_, vars)| !vars.is_empty())
    }

    fn set_record(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        // find the next record for all readers
        let id = record.id();

        for (rdr, vars) in &mut self.readers {
            // find the next record
            let opt_record = rdr.find_next(id)?;

            // copy to symbol table
            for (symbol_id, var) in vars.iter() {
                match var {
                    MetaVarKind::Col(i, ref allow_missing) => {
                        let sym = symbols.get_mut(*symbol_id);
                        if let Some(rec) = opt_record {
                            if let Some(text) = rec.get(*i) {
                                sym.inner_mut().set_text(text);
                            } else {
                                if !allow_missing {
                                    return fail!(
                                        "Column no. {} not found in metadata entry for record '{}'",
                                        *i + 1,
                                        String::from_utf8_lossy(id)
                                    );
                                }
                                sym.set_none();
                            }
                        } else {
                            if !allow_missing {
                                return fail!(
                                    "ID '{}' not found in metadata file '{}'. Use the `opt_meta(field)` function \
                                    instead of `meta(field)` if you expect missing entries.",
                                    String::from_utf8_lossy(id),
                                    rdr.path
                                );
                            }
                            sym.set_none();
                        }
                    }
                    MetaVarKind::Exists => {
                        symbols
                            .get_mut(*symbol_id)
                            .inner_mut()
                            .set_bool(opt_record.is_some());
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct MetaReader {
    path: String,
    rdr: Reader<Box<dyn io::Read + Send>>,
    has_header: bool, // user choice overriding auto-detection
    header: Option<HashMap<String, usize>>,
    current_record: ByteRecord,
    // object doing the ID lookup
    finder: IdFinder,
    id_col: u32,
}

// stub
impl fmt::Debug for MetaReader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MetaReader {{ path: \"{}\", ... }}", self.path)
    }
}

impl MetaReader {
    pub fn new(path: &str, delim: Option<u8>, dup_ids: bool) -> CliResult<Self> {
        let info = FileInfo::from_path(path, FormatVariant::Tsv, false);
        let io_reader = get_io_reader(&InputKind::from_str(path)?, info.compression)
            .map_err(|e| format!("Could not open metadata file '{}': {}", path, e))?;
        let delim = delim.unwrap_or(match info.format {
            FormatVariant::Csv => b',',
            _ => b'\t',
        });
        Ok(Self {
            path: path.to_string(),
            rdr: ReaderBuilder::new()
                .delimiter(delim)
                .has_headers(false)
                .flexible(true)
                .from_reader(io_reader),
            has_header: false,
            header: None,
            current_record: ByteRecord::new(),
            finder: IdFinder::new(!dup_ids),
            id_col: 0,
        })
    }

    pub fn set_id_col(&mut self, id_col: u32) {
        self.id_col = id_col;
    }

    pub fn set_has_header(&mut self, has_header: bool) {
        self.has_header = has_header;
    }

    fn get_col_index(&mut self, col: &str) -> Result<usize, String> {
        if !self.has_header {
            // assuming column indices
            if let Ok(idx) = col.parse::<usize>() {
                if idx == 0 {
                    return fail!(
                        "Invalid metadata column access: column numbers must be > 0 (file: '{}')",
                        self.path
                    );
                }
                if self.header.is_none() {
                    return Ok(idx - 1);
                }
            }
        }

        // switch to header mode
        // look up column name
        self.has_header = true;
        if self.header.is_none() {
            let map = self
                .rdr
                .headers()
                .map_err(|e| {
                    format!(
                        "Error reading the header of the metadata file ({}): {}",
                        self.path, e
                    )
                })?
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), i))
                .collect();
            self.header = Some(map);
        }

        if let Some(i) = self.header.as_mut().unwrap().get(col) {
            return Ok(*i);
        }

        fail!(
            "Column '{}' not found in header of metadata file '{}'",
            col,
            self.path
        )
    }

    fn find_next(&mut self, id: &[u8]) -> Result<Option<&ByteRecord>, String> {
        // read header record once (if present)
        if self.has_header {
            self.rdr
                .read_byte_record(&mut self.current_record)
                .map_err(|e| {
                    format!(
                        "Error reading metadata header from file '{}': {}",
                        self.path, e
                    )
                })?;
            self.has_header = false;
        }

        // find the next record
        let exists = self
            .finder
            .find(id, self.id_col, &mut self.rdr, &mut self.current_record)?;

        if exists {
            Ok(Some(&self.current_record))
        } else {
            Ok(None)
        }
    }
}

/// Initial hash map index capacity
const IDX_INITIAL_CAP: usize = 5000;

/// Number of records for which to check for duplicate IDs
const DUPLICATE_CHECK_N: usize = 10000;
/// Object reponsible for looking up a metadata record with the given ID.
/// First, it tries reading records in order, but switches to a hash map
/// approach for storing records that are already read.
pub struct IdFinder {
    in_sync: bool,
    started_in_sync: bool,
    meta_map: HashMap<Vec<u8>, ByteRecord>,
    initial_seq_ids: HashSet<Vec<u8>>,
    dup_reported: bool,
}

impl IdFinder {
    pub fn new(in_sync: bool) -> Self {
        let mut meta_map = HashMap::default();
        if !in_sync {
            meta_map.reserve(IDX_INITIAL_CAP);
        }
        Self {
            in_sync,
            started_in_sync: in_sync,
            meta_map,
            initial_seq_ids: HashSet::default(),
            dup_reported: false,
        }
    }

    fn find<R: io::Read>(
        &mut self,
        seq_id: &[u8],
        meta_id_col: u32,
        meta_rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
    ) -> Result<bool, String> {
        // first, check the record map for the given ID
        if !self.in_sync {
            // If the reader switched from in sync -> hash map index parsing,
            // we check at least `DUPLICATE_CHECK_SIZE` sequence IDs for
            // duplication (since duplicates are problematic in this case).
            // These will not detect all duplicates, but
            // (1) ID duplicates are unusual and rather a corner case, and
            // (2) checking all IDs would be slower
            if self.started_in_sync {
                let n = self.initial_seq_ids.len();
                if n <= DUPLICATE_CHECK_N {
                    if n == 0 {
                        self.initial_seq_ids.reserve(DUPLICATE_CHECK_N / 2);
                    }
                    if !self.initial_seq_ids.insert(seq_id.to_owned()) {
                        return fail!(
                            "Found duplicate sequence ID: '{}'. \
                            Please specify --dup-ids, otherwise the `meta` and `has_meta` functions \
                            can lead to errors and `opt_meta` may incorrectly report.",
                            String::from_utf8_lossy(seq_id)
                        );
                    }
                    if n == DUPLICATE_CHECK_N {
                        // clear hash set to save memory
                        self.initial_seq_ids.clear();
                    }
                }
            }

            // now return if sequence ID in index
            if let Some(found) = self.meta_map.get(seq_id) {
                rec.clone_from(found);
                return Ok(true);
            }
        }

        // if the metadata was not found, read until the ID is encountered
        while meta_rdr.read_byte_record(rec).map_err(|e| {
            format!(
                "Error reading metadata record for '{}': {}",
                String::from_utf8_lossy(seq_id),
                e
            )
        })? {
            let row_id = rec.get(meta_id_col as usize).ok_or_else(|| {
                format!(
                    "ID Column not found in record no. {} at line {}",
                    meta_rdr.position().record() + 1,
                    meta_rdr.position().line()
                )
            })?;

            if self.in_sync {
                // if the first encountered entry is the correct one, we are
                // still in sync
                if row_id == seq_id {
                    return Ok(true);
                }
                // Otherwise, proceed to reading & building a hash map index
                // until an entry is found.
                // (but previous entries from 'in sync' reading are no available)
                self.in_sync = false;
                self.meta_map.reserve(IDX_INITIAL_CAP);
            }

            match self.meta_map.entry(row_id.to_owned()) {
                Entry::Vacant(e) => {
                    e.insert(rec.clone());
                }
                Entry::Occupied(_) => {
                    if !self.dup_reported {
                        let extra = " Additionally make sure to specify --dup-ids \
                        if you expect duplicate IDs in *sequence records*!";
                        eprintln!(
                            "Found duplicate IDs in associated metadata (first: {}). \
                            Only the first entry is used.{}",
                            String::from_utf8_lossy(row_id),
                            if self.started_in_sync { extra } else { "" }
                        );
                        self.dup_reported = true;
                    }
                }
            }

            if row_id == seq_id {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
