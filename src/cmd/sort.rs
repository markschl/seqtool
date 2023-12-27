use clap::Parser;
use ordered_float::OrderedFloat;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::vec::VecFactory;
use crate::opt::CommonArgs;
use crate::var::symbols::{SymbolTable, VarType};
use crate::var::varstring::{DynValue, VarString};
use crate::var::{Func, VarBuilder, VarHelp, VarProvider};

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

    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Hash, Clone)]
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

pub fn run(cfg: Config, args: &SortCommand) -> CliResult<()> {
    let force_numeric = args.numeric;

    let m = Box::new(KeyVars::default());
    cfg.writer_with_custom(Some(m), |writer, io_writer, vars| {
        // assemble key
        let (var_key, _vtype) = vars.build(|b| VarString::var_or_composed(&args.key, b))?;
        // we cannot know the exact length of the input, we just initialize
        // with capacity that should at least hold some records, while still
        // not using too much memory
        let mut records = Vec::with_capacity(10000);
        let mut record_buf_factory = VecFactory::new();
        let mut key_buf = Vec::new();

        cfg.read(vars, |record, vars| {
            let key = vars.custom_mod::<KeyVars, _>(|key_mod, symbols| {
                let key = var_key
                    .get_dyn(symbols, record, &mut key_buf, force_numeric)?
                    .into();
                if let Some(m) = key_mod {
                    m.set(&key, symbols);
                }
                Ok(key)
            })?;

            let record_out = record_buf_factory.fill_vec(|out| writer.write(&record, out, vars))?;
            records.push((key, record_out));
            Ok(true)
        })?;
        // now we have all the records in memory, so we can sort them
        if !args.reverse {
            records.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        } else {
            records.sort_by(|(k1, _), (k2, _)| k2.cmp(k1));
        }
        // then write to output
        for (_, buf) in records {
            io_writer.write_all(&buf)?;
        }
        Ok(())
    })
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
