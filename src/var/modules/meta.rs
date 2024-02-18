use std::collections::hash_map::Entry;
use std::fmt;
use std::io;
use std::str::FromStr;

use csv::{self, ByteRecord, Reader, ReaderBuilder};
use fxhash::{FxHashMap, FxHashSet};
use strum_macros::{Display, EnumString};

use crate::error::CliResult;
use crate::io::{
    input::{get_io_reader, InputKind},
    FileInfo, FormatVariant, QualConverter, Record,
};
use crate::var::{
    attr::Attrs,
    func::{ArgValue, Func},
    symbols::{SymbolTable, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct MetaInfo;

impl VarProviderInfo for MetaInfo {
    fn name(&self) -> &'static str {
        "Associated metadata from delimited text files"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(
                meta [ (column_number), (column_name) ] =>
                "Obtain a value from a column (1, 2, 3, etc. or 'name') of the delimited text file \
                supplied with `-m` or `--meta`. \
                Missing entries are not allowed. \
                Field may be quoted or not (but quoting is required in Javascript expressions)."
            ),
            var_info!(
                meta [ (file_num, col_num), (file_num, col_name) ] =>
                "If there are multiple metadata files supplied with -m/--meta \
                (`-m file1 -m file2 -m file3, ...`), the specific file can be referenced \
                by supplying `<file-number>` (1, 2, 3, ...) as first argument, followed by  \
                the column number. If only a single file is supplied, only the column \
                number or name is required (see above use case). \
                Missing entries are not allowed."
            ),
            var_info!(
                opt_meta [ (column_number), (column_name) ] =>
                "Like `meta(column_number)` or `meta(column_name)`, but not every sequence record \
                is required to have an associated metadata row matching the given record ID. \
                Missing values will result in an empty string in the output \
                (or 'undefined' in JavaScript expressions)."
            ),
            var_info!(
                opt_meta [ (file_num, col_num), (file_num, col_name) ] =>
                "Like `meta(file_num, column)`, \
                but missing metadata entries are possible."
            ),
            var_info!(
                has_meta [ (), (file_number) ] =>
                "Returns true if the given record has a metadata entry with the same ID in the \
                in the given file. In case of multiple files, the file number \
                must be supplied as an argument."
            ),
        ]
    }

    fn desc(&self) -> Option<&'static str> {
        Some(
            "The functions `meta`, `opt_meta` and `has_meta` allow accessing \
            associated metadata in plain delimited text files \
            (optionally compressed, auto-recognized). \
            These files must contain a column with the sequence ID \
            (default: 1st column; change with `--meta-idcol`).\n\
            The column delimiter is guessed if possible (override with `--meta-delim`): \
            `.csv` is interpreted as comma(,)-delimited, `.tsv`/`.txt` or other (unknown) \
            extensions are assumed to be tab-delimited.\n\
            The first line is implicitly assumed to contain \
            column names if a non-numeric field name is requested, e.g. `meta(fieldname)`. \
            Use `--meta-header` to explicitly enable header lines \
            (necessary if column names are numbers).\n\n\
            Multiple metadata files can be supplied (`-m file1 -m file2 -m file3 ...`) and \
            are addressed via `file-num` (see function descriptions).\n\n\
            For maximum performance, provide metadata records in the same order as \
            sequence records. \
            \n\n\
            *Note:* Specify `--dup-ids` if the sequence input is expected to contain \
            duplicate IDs (which is rather unusual). See the help page (`-h/--help`) \
            for more information. 
            ",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Add taxonomic lineages stored in a GZIP-compressed TSV file (column no. 2) \
                to the FASTA headers, as description after the first space. \
                 The resulting headers look like this: `>id1 k__Fungi;p__Basidiomycota;c__...`",
                "st set -m taxonomy.tsv.gz -d '{meta(2)}' input.fa > output.fa",
            ),
            (
                "Integrate metadata from an Excel-generated CSV file \
                (with semicolon delimiter and column names) \
                into sequence records in form of a header attribute (`-a/--attr`); \
                resulting output: >id1 info=somevalue",
                "st pass -m metadata.csv --meta-sep ';' -a 'info={meta(column-name)}' input.fa > output.fa",
            ),
            (
                "Extract sequences with coordinates stored in a BED file \
                (equivalent to `bedtools getfasta`)",
                "st trim -m coordinates.bed -0 {meta(2)}..{meta(3)} input.fa > output.fa",
            ),
            (
                "Filter sequences by ID, keeping only those present in the given text file",
                "st filter -m selected_ids.txt 'has_meta()' input.fa > output.fa",
            ),
        ])
    }
}

#[derive(Debug)]
enum MetaVarType {
    // column index, allow missing
    Col(usize, bool),
    Exists,
}

#[derive(Debug)]
pub struct MetaVars {
    // metadata readers together with registered variabbles (var_id, col_idx)
    readers: Vec<(MetaReader, Vec<(usize, MetaVarType)>)>,
}

impl MetaVars {
    pub fn new<I: IntoIterator<Item = P>, P: AsRef<str>>(
        paths: I,
        delim: Option<u8>,
        dup_ids: bool,
    ) -> CliResult<Self> {
        let readers = paths
            .into_iter()
            .map(|path| Ok((MetaReader::new(path.as_ref(), delim, dup_ids)?, Vec::new())))
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
}

impl VarProvider for MetaVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &MetaInfo
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        // Get function type as enum to avoid multiple string comparisons
        #[derive(Debug, Display, EnumString, PartialEq)]
        #[strum(serialize_all = "snake_case")]
        enum _VarType {
            Meta,
            OptMeta,
            HasMeta,
        }
        // note: unwrap is ok, since only functions with names present in MetaInfo are supplied
        let var = _VarType::from_str(&func.name).unwrap();

        // there should be at least one metadata file
        if self.readers.is_empty() {
            return fail!("The '{}' function is used, but no metadata source was supplied with -m/--meta <file>.", func.name);
        }
        // get file number
        // obtain and validate file number
        let (file_num, arg_offset) = if var == _VarType::HasMeta {
            (func.opt_arg_as::<usize>(0).transpose()?, 0)
        } else if func.args.len() == 2 {
            (Some(func.arg_as::<usize>(0)?), 1)
        } else {
            (None, 0)
        };
        let file_num = if self.readers.len() == 1 {
            file_num.unwrap_or(1)
        } else {
            file_num.ok_or_else(|| format!(
                "The '{}' function does not have enough arguments. Please specify the file number as first argument, \
                since multiple metadata files were supplied with -m/--meta.",
                func.name
            ))?
        };
        if file_num == 0 {
            return fail!("Invalid metadata file no. requested: 0",);
        }
        if file_num > self.readers.len() {
            return fail!(
                "Metadata file no. {} was requested by `{}`, \
                but only {} metadata source(s) were supplied with -m/--meta",
                file_num,
                func.name,
                self.readers.len()
            );
        }
        let (rdr, vars) = &mut self.readers[file_num - 1];

        let (var, vtype) = match var {
            _VarType::HasMeta => (MetaVarType::Exists, VarType::Bool),
            _VarType::Meta | _VarType::OptMeta => {
                let col = func.arg(arg_offset);
                let i = rdr.get_col_index(col)?;
                (MetaVarType::Col(i, var == _VarType::OptMeta), VarType::Text)
            }
        };
        vars.push((b.symbol_id(), var));
        Ok(Some(vtype))
    }

    fn has_vars(&self) -> bool {
        self.readers.iter().any(|(_, vars)| !vars.is_empty())
    }

    fn set(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attrs,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        // find the next record for all readers
        let id = record.id_bytes();

        for (rdr, vars) in &mut self.readers {
            // find the next record
            let opt_record = rdr.find_next(id)?;

            // copy to symbol table
            for (var_id, var) in vars {
                match var {
                    MetaVarType::Col(i, ref allow_missing) => {
                        let sym = symbols.get_mut(*var_id);
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
                    MetaVarType::Exists => {
                        symbols
                            .get_mut(*var_id)
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
    header: Option<FxHashMap<String, usize>>,
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
        let io_reader = get_io_reader(&InputKind::from_str(path)?, info.compression)?;
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

    fn get_col_index(&mut self, col: &str) -> CliResult<usize> {
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
        let col_name: String = ArgValue::from_str(col)
            .ok_or_else(|| format!("Invalid metadata column: '{}' (file: '{}')", col, self.path))?;
        self.has_header = true;
        if self.header.is_none() {
            let map = self
                .rdr
                .headers()?
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), i))
                .collect();
            self.header = Some(map);
        }

        if let Some(i) = self.header.as_mut().unwrap().get(&col_name) {
            return Ok(*i);
        }

        fail!(
            "Column '{}' not found in header of metadata file '{}'",
            col_name,
            self.path
        )
    }

    fn find_next(&mut self, id: &[u8]) -> CliResult<Option<&ByteRecord>> {
        // read header record once (if present)
        if self.has_header {
            self.rdr.read_byte_record(&mut self.current_record)?;
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
    meta_map: FxHashMap<Vec<u8>, ByteRecord>,
    initial_seq_ids: FxHashSet<Vec<u8>>,
    dup_reported: bool,
}

impl IdFinder {
    pub fn new(in_sync: bool) -> Self {
        let mut meta_map = FxHashMap::default();
        if !in_sync {
            meta_map.reserve(IDX_INITIAL_CAP);
        }
        Self {
            in_sync,
            started_in_sync: in_sync,
            meta_map,
            initial_seq_ids: FxHashSet::default(),
            dup_reported: false,
        }
    }

    fn find<R: io::Read>(
        &mut self,
        seq_id: &[u8],
        meta_id_col: u32,
        meta_rdr: &mut Reader<R>,
        rec: &mut ByteRecord,
    ) -> CliResult<bool> {
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
        while meta_rdr.read_byte_record(rec)? {
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
