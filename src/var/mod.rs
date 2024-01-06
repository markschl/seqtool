use std::fmt::Debug;
use std::fmt::Display;
use std::fs::File;

use crate::error::CliResult;
use crate::helpers::any::AsAnyMut;
use crate::io::{input::InputOptions, output::OutputOptions, QualConverter, Record};

use self::attr::Attrs;
pub use self::build::*;
use self::func::Func;
use self::symbols::{SymbolTable, VarType};

pub mod attr;
pub mod build;
pub mod func;
pub mod modules;
pub mod symbols;
pub mod varstring;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VarOpts {
    // metadata
    pub lists: Vec<String>,
    pub list_delim: char,
    pub has_header: bool,
    pub unordered: bool,
    pub id_col: u32,
    pub allow_missing: bool,
    // attributes
    pub attr_opts: AttrOpts,
    // expressions
    pub expr_init: Option<String>,
    // Used to remember that the variable help page has to be returned
    pub var_help: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct AttrOpts {
    pub delim: char,
    pub value_delim: char,
}

impl Default for AttrOpts {
    fn default() -> Self {
        AttrOpts {
            delim: ' ',
            value_delim: '=',
        }
    }
}

pub trait VarProvider: Debug + AsAnyMut {
    fn help(&self) -> &dyn VarHelp;

    /// Try registering a variable / "function" with a module.
    /// If the function/variable is not found in the given module,
    /// the implementor should return Ok(None).
    fn register(
        &mut self,
        func: &Func,
        vars: &mut VarBuilder,
    ) -> CliResult<Option<Option<VarType>>>;

    fn has_vars(&self) -> bool;

    /// Is it allowed to use this module's variables/functions from within
    /// another module?
    /// (currently the case with expressions, in theory
    /// any kind of dependency, e.g. if function args should be evaluated)
    /// This is relevant to know for custom variable providers, whose value is only added
    /// at a later stage and in turn depends on other variables/expressions,
    /// making it impossible with the current simple system to represent this kind
    /// of cyclic relationships.
    fn allow_dependent(&self) -> bool {
        true
    }

    /// Supplies a new record, allowing the variable provider to obtain the necessary
    /// information and add it to the metadata object (usually the symbol table).
    fn set(
        &mut self,
        _rec: &dyn Record,
        _sym: &mut SymbolTable,
        _attr: &mut Attrs,
        _qc: &mut QualConverter,
    ) -> CliResult<()> {
        Ok(())
    }

    fn init(&mut self, _: &OutputOptions) -> CliResult<()> {
        Ok(())
    }

    /// Called on every new input (STDIN or file)
    fn new_input(&mut self, _: &InputOptions) -> CliResult<()> {
        Ok(())
    }
}

impl VarProvider for Box<dyn VarProvider> {
    fn help(&self) -> &dyn VarHelp {
        (**self).help()
    }

    fn register(
        &mut self,
        func: &Func,
        vars: &mut VarBuilder,
    ) -> CliResult<Option<Option<VarType>>> {
        (**self).register(func, vars)
    }
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn allow_dependent(&self) -> bool {
        (**self).allow_dependent()
    }
    fn set(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        attrs: &mut Attrs,
        qual_converter: &mut QualConverter,
    ) -> CliResult<()> {
        (**self).set(record, symbols, attrs, qual_converter)
    }
    fn init(&mut self, o: &OutputOptions) -> CliResult<()> {
        (**self).init(o)
    }
    fn new_input(&mut self, o: &InputOptions) -> CliResult<()> {
        (**self).new_input(o)
    }
}

pub trait VarHelp: Debug {
    fn name(&self) -> &'static str;
    fn usage(&self) -> Option<&'static str> {
        None
    }
    fn desc(&self) -> Option<&'static str> {
        None
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        None
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        None
    }
}

impl Display for &dyn VarHelp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(u) = self.usage() {
            writeln!(f, "{}. Usage: {}", self.name(), u).unwrap();
            let w = self.name().len() + 9 + u.len().min(80);
            writeln!(f, "{1:-<0$}", w, "").unwrap();
        } else {
            writeln!(f, "{}\n{2:-<1$}", self.name(), self.name().len(), "").unwrap();
        }
        if let Some(desc) = self.desc() {
            for d in textwrap::wrap(desc, 80) {
                writeln!(f, "{}", d).unwrap();
            }
            writeln!(f).unwrap();
        }
        if let Some(v) = self.vars() {
            for &(name, desc) in v {
                for (i, d) in textwrap::wrap(desc, 68).into_iter().enumerate() {
                    let n = if i == 0 { name } else { "" };
                    writeln!(f, "{: <12} {}", n, d).unwrap();
                }
            }
            writeln!(f).unwrap();
        }
        if let Some(examples) = self.examples() {
            writeln!(f, "Example{}:", if examples.len() > 1 { "s" } else { "" }).unwrap();
            for &(desc, example) in examples {
                let mut desc = desc.to_string();
                desc.push(':');
                for d in textwrap::wrap(&desc, 80) {
                    writeln!(f, "{}", d).unwrap();
                }
                writeln!(f, "> {}", example).unwrap();
            }
        }
        writeln!(f).unwrap();
        Ok(())
    }
}

pub fn init_vars(
    modules: &mut Vec<Box<dyn VarProvider>>,
    custom_mod: Option<Box<dyn VarProvider>>,
    opts: &VarOpts,
    out_opts: &OutputOptions,
) -> CliResult<()> {
    // the custom module needs to be inserted early
    // at least before the expression module
    if let Some(m) = custom_mod {
        modules.push(m);
    }

    // lists
    for (i, list) in opts.lists.iter().enumerate() {
        let csv_file = File::open(list).map_err(|e| format!("Error opening '{}': {}", list, e))?;
        if opts.unordered {
            let finder = modules::list::Unordered::new();
            modules.push(Box::new(
                modules::list::ListVars::new(
                    i + 1,
                    opts.lists.len(),
                    csv_file,
                    finder,
                    opts.list_delim as u8,
                )
                .id_col(opts.id_col)
                .has_header(opts.has_header)
                .allow_missing(opts.allow_missing),
            ));
        } else {
            let finder = modules::list::SyncIds;
            modules.push(Box::new(
                modules::list::ListVars::new(
                    i + 1,
                    opts.lists.len(),
                    csv_file,
                    finder,
                    opts.list_delim as u8,
                )
                .id_col(opts.id_col)
                .has_header(opts.has_header)
                .allow_missing(opts.allow_missing),
            ));
        }
    }

    // other modules
    modules.push(Box::new(modules::builtins::BuiltinVars::new()));
    modules.push(Box::new(modules::stats::StatVars::new()));
    modules.push(Box::new(modules::attr::AttrVars::new()));

    #[cfg(feature = "expr")]
    modules.push(Box::new(modules::expr::ExprVars::new(
        opts.expr_init.as_deref(),
    )?));

    // make aware of output options
    for m in modules {
        m.init(out_opts)?;
    }

    Ok(())
}
