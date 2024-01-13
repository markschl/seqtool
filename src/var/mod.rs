use std::fmt::{self, Debug, Display, Write};

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
    pub metadata_sources: Vec<String>,
    pub meta_delim_override: Option<u8>,
    pub has_header: bool,
    pub meta_id_col: u32,
    pub meta_dup_ids: bool,
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

impl VarHelp for Box<dyn VarHelp> {
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn usage(&self) -> Option<&'static str> {
        (**self).usage()
    }
    fn desc(&self) -> Option<&'static str> {
        (**self).desc()
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        (**self).vars()
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        (**self).examples()
    }
}

impl Display for &dyn VarHelp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(u) = self.usage() {
            writeln!(f, "{}. Usage: {}", self.name(), u)?;
            let w = self.name().len() + 9 + u.len().min(80);
            writeln!(f, "{1:=<0$}", w, "")?;
        } else {
            writeln!(f, "{}\n{2:=<1$}", self.name(), self.name().len(), "")?;
        }
        if let Some(desc) = self.desc() {
            for d in textwrap::wrap(desc, 80) {
                writeln!(f, "{}", d)?;
            }
            writeln!(f)?;
        }
        if let Some(v) = self.vars() {
            for &(name, desc) in v {
                for (i, d) in textwrap::wrap(desc, 68).into_iter().enumerate() {
                    if i == 0 {
                        if name.len() < 10 {
                            writeln!(f, "{: <12} {}", name, d)?;
                            continue;
                        } else {
                            writeln!(f, "{}", name)?;
                        }
                    }
                    writeln!(f, "{: <12} {}", "", d)?;
                }
            }
            writeln!(f)?;
        }
        if let Some(examples) = self.examples() {
            let mut ex = "Example".to_string();
            if examples.len() > 1 {
                ex.push('s');
            }
            writeln!(f, "{}", ex)?;
            writeln!(f, "{1:-<0$}", ex.len(), "")?;
            for &(desc, example) in examples {
                let mut desc = desc.to_string();
                desc.push(':');
                for d in textwrap::wrap(&desc, 80) {
                    writeln!(f, "{}", d)?;
                }
                writeln!(f, "> {}", example)?;
                writeln!(f)?;
            }
        }
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
    for (i, path) in opts.metadata_sources.iter().enumerate() {
        modules.push(Box::new(
            modules::meta::MetaVars::new(
                i + 1,
                opts.metadata_sources.len(),
                path,
                opts.meta_delim_override,
                opts.meta_dup_ids,
            )?
            .id_col(opts.meta_id_col)
            .set_has_header(opts.has_header),
        ));
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

pub fn get_var_help(custom_help: Option<Box<dyn VarHelp>>) -> Result<String, fmt::Error> {
    let mut out = "".to_string();
    if let Some(m) = custom_help {
        writeln!(&mut out, "{}", &m as &dyn VarHelp)?;
    }
    writeln!(
        &mut out,
        "{}",
        &modules::builtins::BuiltinHelp as &dyn VarHelp
    )?;
    writeln!(&mut out, "{}", &modules::stats::StatHelp as &dyn VarHelp)?;
    writeln!(&mut out, "{}", &modules::attr::AttrHelp as &dyn VarHelp)?;
    writeln!(&mut out, "{}", &modules::meta::MetaHelp as &dyn VarHelp)?;
    #[cfg(feature = "expr")]
    writeln!(&mut out, "{}", &modules::expr::ExprHelp as &dyn VarHelp)?;
    Ok(out)
}
