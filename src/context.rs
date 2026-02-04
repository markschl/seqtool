use std::borrow::Borrow;
use std::cell::Cell;

use crate::cli::BasicStats;
use crate::error::CliResult;
use crate::io::input::InputConfig;
use crate::io::output::{OutputOpts, WriteFinish};
use crate::io::{IoKind, QualConverter, QualFormat, Record};
use crate::var::{
    attr::{AttrFormat, Attributes},
    modules::VarProvider,
    symbols::SymbolTable,
    VarProviders,
};

/// Object holding metadata (attributes and symbols) for a single sequence record
#[derive(Debug, Clone)]
pub struct RecordMeta {
    pub symbols: SymbolTable,
    pub attrs: Attributes,
}

impl RecordMeta {
    pub fn new(attr_format: AttrFormat) -> RecordMeta {
        RecordMeta {
            symbols: SymbolTable::new(0),
            attrs: Attributes::new(attr_format),
        }
    }

    /// Update data from a new sequence record: parse header attributes and update the symbol table
    pub fn set_record(
        &mut self,
        record: &dyn Record,
        var_providers: &mut VarProviders,
        qc: &mut QualConverter,
    ) -> CliResult<()> {
        if self.attrs.has_read_attrs() {
            self.attrs.parse(record);
        }
        var_providers.update_symbols(record, &mut self.symbols, &self.attrs, qc)?;
        Ok(())
    }
}

/// Object providing access to variables/functions, header attributes and
/// methods to convert quality scores.
/// It should be initialized with each new sequence record, which will then
/// be passed to all `VarProvider`s, which will then in turn fill in the
/// symbol table with values for all necessary variables.
#[derive(Debug)]
pub struct SeqContext {
    pub var_providers: VarProviders,
    /// record metadata (number of slots predefined in `Config`)
    pub meta: Vec<RecordMeta>,
    pub qual_converter: QualConverter,
    // needed by `io_writer_from_path`
    // TODO: another copy is in `Config` due to borrowing issues
    output_opts: OutputOpts,
    /// Set to `true` as soon as STDOUT is effectively used
    stdout_in_use: Cell<bool>,
    // for statistics
    pub n_records: u64,
}

impl SeqContext {
    pub fn new(
        attr_format: AttrFormat,
        qual_format: QualFormat,
        var_providers: VarProviders,
        output_opts: OutputOpts,
    ) -> Self {
        Self {
            var_providers,
            meta: vec![RecordMeta::new(attr_format)],
            qual_converter: QualConverter::new(qual_format),
            output_opts,
            stdout_in_use: Cell::new(false),
            n_records: 0,
        }
    }

    /// Initialize context with a new input
    /// (done in Config while reading)
    pub fn init_input(&mut self, cfg: &InputConfig) -> CliResult<()> {
        self.var_providers.init_input(cfg)
    }

    /// Initialize context with a new sequence record
    /// (done in Config while reading, or manually with Config::read_alongside)
    ///
    /// `meta_slot` is the slot number in `meta`
    #[inline(always)]
    pub fn set_record(&mut self, record: &dyn Record, meta_slot: usize) -> CliResult<()> {
        self.meta[meta_slot].set_record(record, &mut self.var_providers, &mut self.qual_converter)
    }

    #[inline(always)]
    pub fn increment_record(&mut self) {
        self.n_records += 1;
    }

    // #[inline(always)]
    // pub fn set_record(&mut self, record: &dyn Record, data_i: usize) -> CliResult<()> {
    //     let data = if data_i == 0 {
    //         &mut self.record_data
    //     } else {
    //         &mut self.more_record_data[data_i - 1]
    //     };
    //     data    .set_record(record, &mut self.var_providers, &mut self.qual_converter)
    // }

    /// Read-only shortcut to the symbol table of the first data slot
    pub fn symbols(&self) -> &SymbolTable {
        &self.meta[0].symbols
    }

    /// Gives access to the custom (command-specific) variable provider
    /// in a closure, along with the mutable symtol table,
    /// but only *if* present (any variables registered to it).
    pub fn with_custom_varmod<M, O>(
        &mut self,
        meta_slot: usize,
        func: impl FnOnce(&mut M, &mut SymbolTable) -> O,
    ) -> Option<O>
    where
        M: VarProvider + 'static,
    {
        self.var_providers
            .custom_vars()
            .map(|v| func(v, &mut self.meta[meta_slot].symbols))
    }

    #[cold]
    pub fn check_stdout(&self, kind: &IoKind) -> CliResult<()> {
        if kind == &IoKind::Stdio {
            if self.stdout_in_use.get() {
                return fail!("Cannot write two different sources to STDOUT (-)");
            }
            self.stdout_in_use.set(true);
        }
        Ok(())
    }

    /// Returns an I/O writer (of type WriteFinish) directly without any scope
    /// taking care of cleanup.
    /// The caller is responsible for invoking finish() on the writer when done.
    /// The returned writer may be a compressed writer if configured accordingly
    /// using CLI options or deduced from the output path extension.
    /// Writing in a background thread is not possible, since that
    /// would require a scoped function.
    ///
    /// This method is part of `Context` and not `Config` because it is only
    /// needed in cases where multiple writers have to be dynamically constructed
    /// while running the reader. To use the 'standard' configured output, use
    /// `Config::with_io_writer`.
    ///
    // TODO: this is mostly useful for the 'split' command, which always has the
    // same output format and compression settings. It may not be flexible enough
    // for all future uses.
    pub fn io_writer<K>(&self, kind: K) -> CliResult<Box<dyn WriteFinish>>
    where
        K: Borrow<IoKind>,
    {
        let kind = kind.borrow();
        self.check_stdout(kind)?;
        let w = kind.io_writer(&self.output_opts)?;
        Ok(w)
    }

    pub fn get_stats(&self) -> BasicStats {
        BasicStats {
            n_records: self.n_records,
        }
    }
}
