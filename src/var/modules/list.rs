use std::collections::hash_map::Entry;
use std::fmt;
use std::io;
use std::str::FromStr;

use csv::{self, ByteRecord, Reader, ReaderBuilder};
use fxhash::{FxHashMap, FxHashSet};

use crate::error::CliResult;
use crate::io::{
    input::{get_io_reader, InputKind},
    FileInfo, FormatVariant, QualConverter, Record,
};
use crate::var::{
    attr::Attrs,
    func::{ArgValue, Func},
    symbols::{SymbolTable, VarType},
    VarBuilder, VarHelp, VarProvider,
};

#[derive(Debug)]
pub struct MetaHelp;

impl VarHelp for MetaHelp {
    fn name(&self) -> &'static str {
        "Associated metadata from delimited text files"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "meta(field-number) or meta(field-name) = meta('field-name') = meta(\"field-name\")",
                "Obtain a value from a column of the delimited text file. \
                Missing entries are not allowed for any sequence record. \
                Field names can be quoted (but this is not required).",
            ),
            (
                "meta(file-num, field-num) or meta(file-num, field-name)",
                "Obtain a value from a column of the text file no. <file-num>. \
                The numbering (1, 2, 3, etc.) reflects the order in which the \
                files were provided in the commandline (-m file1 -m file2 -m file3). \
                Missing entries are not allowed. Field name quoting is optional.",
            ),
            (
                "opt_meta(field-number) or opt_meta(field-name)",
                "Like meta(field-number) or meta(field-name), but not all sequence records \
                are required to have an associated metadata value. \
                Missing values will result in an empty string or 'undefined' \
                in JavaScript expressions.",
            ),
            (
                "opt_meta(file-num, field-num) or opt_meta(file-num, field-name)",
                "Like meta(file-num, field-num) / meta(file-num, field-name), \
                but missing metadata will result in an empty string \
                (or 'undefined' in expressions) instead of an error.",
            ),
            (
                "has_meta or has_meta() or has_meta(file-num)",
                "Returns true if the given record has a metadata entry in the \
                in the given file. In case of multiple files, the file number \
                can be supplied as an argument.",
            ),
        ])
    }

    fn desc(&self) -> Option<&'static str> {
        Some(
            "The `meta`, `opt_meta` and `has_meta` variables/functions allow accessing \
            associated metadata in plain delimited text files \
            (optionally compressed, auto-recognized). \
            These files must contain a column with the sequence ID \
            (default: 1st column; change with --meta-idcol). \
            The column delimiter is guessed if possible (override with --meta-delim): \
            '.csv' is interpreted as comma(,)-delimited, '.tsv'/'.txt' or other (unknown) \
            extensions are assumed to be tab-delimited. \
            The first line is implicitly assumed to contain \
            column names if a non-numeric field name is requested, e.g. `meta(fieldname)`. \
            Use --meta-header to explicitly enable header lines \
            (necessary if column names are numbers). \
            Multiple metadata files can be supplied (-m file1 -m file2 -m file3...) and \
            are addressed via 'file-num' (see function descriptions). \
            For maximum performance, provide metadata records in the same order as \
            sequence records. \
            \n\
            *Note:* Specify '--dup-ids' if the sequence input is expected to contain \
            duplicate IDs (which is rather unusual). See the help page (-h/--help) \
            for more information. 
            ",
        )
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Add taxonomic lineages stored in the second column of a GZIP-compressed \
                 TSV file to the FASTA headers, as description after the first space. \
                 The resulting headers look like this: '>id1 k__Fungi;p__Basidiomycota;c__...'",
                "st set -m taxonomy.tsv.gz -d '{meta(2)}' input.fa > output.fa",
            ),
            (
                "Integrate metadata from an Excel-generated CSV file \
                (with semicolon delimiter and column names) \
                into sequence records in form of a header attribute (-a/--attr) \
                (possible output: >id1 info=somevalue)",
                "st pass -m metadata.csv --meta-sep ';' -a 'info={meta(column-name)}' input.fa > output.fa",
            ),
            (
                "Extract sequences with coordinates stored in a BED file",
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

pub struct MetaVars {
    // file
    file_num: usize,
    total_files: usize,
    path: String,
    // CSV reader
    rdr: Reader<Box<dyn io::Read + Send>>,
    has_header: bool, // user choice overriding auto-detection
    header: Option<FxHashMap<String, usize>>,
    current_record: ByteRecord,
    // object doing the ID lookup
    finder: IdFinder,
    id_col: u32,
    // registered variables: (var_id, col_idx)
    vars: Vec<(usize, MetaVarType)>,
}

impl MetaVars {
    pub fn new(
        file_num: usize,
        total_files: usize,
        path: &str,
        delim: Option<u8>,
        dup_ids: bool,
    ) -> CliResult<Self> {
        let info = FileInfo::from_path(path, FormatVariant::Tsv, false);
        let io_reader = get_io_reader(&InputKind::from_str(path)?, info.compression)?;
        let delim = delim.unwrap_or(match info.format {
            FormatVariant::Csv => b',',
            _ => b'\t',
        });
        Ok(Self {
            file_num,
            total_files,
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
            vars: vec![],
        })
    }

    pub fn id_col(mut self, id_col: u32) -> Self {
        self.id_col = id_col;
        self
    }

    pub fn set_has_header(mut self, has_header: bool) -> Self {
        self.has_header = has_header;
        self
    }

    fn get_col_index(&mut self, col: &str) -> CliResult<usize> {
        if !self.has_header {
            // assuming column indices
            if let Ok(idx) = col.parse::<usize>() {
                if idx == 0 {
                    return fail!("Metadata columns must be > 0 (file: {})", self.path);
                }
                if self.header.is_none() {
                    return Ok(idx - 1);
                }
            }
        }

        // switch to header mode
        // look up column name
        let col_name: String = ArgValue::from_str(col)
            .ok_or_else(|| format!("Invalid metadata column: {} (file: {})", col, self.path))?;
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
            "Metadata column '{}' not found in '{}'",
            col_name,
            self.path
        )
    }

    fn check_file_num(&self, num: usize, func_name: &str) -> CliResult<bool> {
        if num > self.total_files {
            return fail!(
                "Metadata file no. {} was requested by `{}`, \
                but only {} metadata sources were supplied with -m/--meta",
                num,
                func_name,
                self.total_files
            );
        }
        if num == 0 {
            return fail!("Invalid metadata file no. requested: 0.",);
        }
        if num != self.file_num {
            // another file, not the current one
            return Ok(false);
        }
        Ok(true)
    }
}

impl VarProvider for MetaVars {
    fn help(&self) -> &dyn VarHelp {
        &MetaHelp
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<Option<VarType>>> {
        let (var, vtype) = match func.name.as_ref() {
            "has_meta" => {
                func.ensure_arg_range(0, 1)?;
                let file_num = func.arg_as::<usize>(0).transpose()?.unwrap_or(1);
                if !self.check_file_num(file_num, &func.name)? {
                    return Ok(None);
                }
                (MetaVarType::Exists, VarType::Bool)
            }
            "meta" | "opt_meta" => {
                debug_assert!(self.file_num != 0);
                let (file_num, col) = match func.num_args() {
                    1 => (1, func.arg(0).unwrap()),
                    2 => (
                            func.arg_as::<i64>(0).unwrap()? as usize,
                            func.arg(1).unwrap()
                        ),
                    _ => return fail!("`meta`/`opt_meta` accept only 1 or 2 arguments: meta(field) or meta(file-num, field)")
                };
                if !self.check_file_num(file_num, &func.name)? {
                    return Ok(None);
                }

                let i = self.get_col_index(col)?;
                (MetaVarType::Col(i, func.name == "opt_meta"), VarType::Text)
            }
            _ => return Ok(None),
        };

        self.vars.push((b.symbol_id(), var));
        Ok(Some(Some(vtype)))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attrs,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        // read header record once (if present)
        if self.has_header {
            self.rdr.read_byte_record(&mut self.current_record)?;
            self.has_header = false;
        }

        // find the next record
        let id = record.id_bytes();
        let exists = self
            .finder
            .find(id, self.id_col, &mut self.rdr, &mut self.current_record)?;

        // copy to symbol table
        for (var_id, var) in &self.vars {
            match var {
                MetaVarType::Col(i, allow_missing) => {
                    if !allow_missing && !exists {
                        return fail!(
                            "ID '{}' not found in metadata. Use the `opt_meta(field)` function \
                            instead of `meta(field)` if you expect missing entries.",
                            String::from_utf8_lossy(id)
                        );
                    }
                    let sym = symbols.get_mut(*var_id);
                    if let Some(text) = self.current_record.get(*i) {
                        sym.inner_mut().set_text(text);
                    } else {
                        if !allow_missing {
                            return fail!(
                                "Column no. {} not found in metadata entry for '{}'",
                                *i + 1,
                                String::from_utf8_lossy(id)
                            );
                        }
                        sym.set_none();
                    }
                }
                MetaVarType::Exists => {
                    symbols.get_mut(*var_id).inner_mut().set_bool(exists);
                }
            }
        }

        Ok(())
    }
}

// stub
impl fmt::Debug for MetaVars {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MetaVars {{ file_num: {} }}", self.file_num)
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
                        self.initial_seq_ids.reserve(DUPLICATE_CHECK_N/2);
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
                        let extra = " Also make sure to specify --dup-ids \
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
