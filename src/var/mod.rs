use std::fmt::{self, Debug, Display, Formatter, Write};

use itertools::Itertools;

use crate::error::CliResult;
use crate::helpers::any::AsAnyMut;
use crate::io::{input::InputOptions, output::OutputOptions, QualConverter, Record};

use self::attr::{AttrFormat, Attributes};
pub use self::build::*;
use self::func::Func;
use self::symbols::{SymbolTable, VarType};

pub mod attr;
pub mod build;
pub mod func;
pub mod modules;
pub mod symbols;
pub mod varstring;

#[derive(Debug, Clone)]
pub struct VarOpts {
    // metadata
    pub metadata_sources: Vec<String>,
    pub meta_delim_override: Option<u8>,
    pub has_header: bool,
    pub meta_id_col: u32,
    pub meta_dup_ids: bool,
    // attributes
    pub attr_format: AttrFormat,
    // expressions
    pub expr_init: Option<String>,
    // Used to remember that the variable help page has to be returned
    pub var_help: bool,
}

pub trait VarProvider: Debug + AsAnyMut {
    fn info(&self) -> &dyn VarProviderInfo;

    /// Try registering a variable / "function" with a module
    /// and return `Some(VarType)` or `None` if the type is unknown beforehand.
    /// TODO: The `VarType` information is currently not used anywhere, but may in the future
    fn register(&mut self, func: &Func, vars: &mut VarBuilder) -> CliResult<Option<VarType>>;

    fn has_vars(&self) -> bool;

    /// Is it allowed to use this module's variables/functions from within
    /// another module?
    /// (currently the case with expressions, in theory
    /// any kind of dependency, e.g. if function args should be evaluated)
    /// This is relevant to know for custom variable providers, whose value is only added
    /// at a later stage and in turn depends on other variables/expressions,
    /// making it impossible with the current simple system to represent this kind
    /// of cyclic relationships.
    fn allow_nested(&self) -> bool {
        true
    }

    /// Supplies a new record, allowing the variable provider to obtain the necessary
    /// information and add it to the metadata object (usually the symbol table).
    fn set(
        &mut self,
        _rec: &dyn Record,
        _sym: &mut SymbolTable,
        _attr: &mut Attributes,
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
    fn info(&self) -> &dyn VarProviderInfo {
        (**self).info()
    }

    fn register(&mut self, func: &Func, vars: &mut VarBuilder) -> CliResult<Option<VarType>> {
        (**self).register(func, vars)
    }
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn allow_nested(&self) -> bool {
        (**self).allow_nested()
    }
    fn set(
        &mut self,
        record: &dyn Record,
        symbols: &mut SymbolTable,
        attrs: &mut Attributes,
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

pub struct VarInfo {
    pub name: &'static str,
    pub args: &'static [&'static [ArgInfo]],
    pub description: &'static str,
    pub hidden: bool, // hide from help page
}

impl VarInfo {
    fn display_func(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.args.len());
        for args in self.args {
            let f = if args.is_empty() {
                format!("`{}`", self.name)
            } else {
                format!(
                    "`{}({})`",
                    self.name,
                    args.iter().map(|a| a.to_string()).join(", ")
                )
            };
            out.push(f);
        }
        out
    }
}

pub enum ArgInfo {
    Required(&'static str),
    Optional(&'static str),
}

impl Display for ArgInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgInfo::Required(name) => write!(f, "{}", name),
            ArgInfo::Optional(name) => write!(f, "[{}]", name),
        }
    }
}

#[macro_export]
macro_rules! opt_arg {
    ([$a:ident]) => {
        $crate::var::ArgInfo::Optional(stringify!($a))
    };
    ($a:ident) => {
        $crate::var::ArgInfo::Required(stringify!($a))
    };
}

#[macro_export]
macro_rules! var_info {
    ($name:ident [ $( ( $($arg:tt),* ) ),+ ] => $desc:expr) => {
        $crate::var::VarInfo {
            name: stringify!($name),
            args: &[$(&[$($crate::opt_arg!($arg)),*]),*],
            description: $desc,
            hidden: false,
        }
    };
    ($name:ident ( $($arg:tt),* ) => $desc:expr) => {
        var_info!($name [($($arg),*)] => $desc)
    };
    ($name:ident => $desc:expr) => {
        var_info!($name () => $desc)
    };
}

pub trait VarProviderInfo: Debug {
    fn name(&self) -> &'static str;

    fn desc(&self) -> Option<&'static str> {
        None
    }

    fn vars(&self) -> &[VarInfo] {
        &[]
    }

    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        None
    }

    fn format(&self, markdown: bool) -> String {
        if markdown {
            self._format_md().unwrap()
        } else {
            self._format_text().unwrap()
        }
    }

    fn _format_text(&self) -> CliResult<String> {
        let mut out = String::with_capacity(10000);
        writeln!(out, "{}\n{2:=<1$}", self.name(), self.name().len(), "")?;
        if let Some(desc) = self.desc() {
            for d in textwrap::wrap(desc, 80) {
                writeln!(out, "{}", d)?;
            }
            writeln!(out)?;
        }
        if self.vars().iter().any(|v| !v.hidden) {
            for info in self.vars() {
                if !info.hidden {
                    for (i, d) in textwrap::wrap(info.description, 68).into_iter().enumerate() {
                        if i == 0 {
                            let fn_call = info.display_func().join(" or ");
                            if fn_call.len() < 10 {
                                writeln!(out, "{: <12} {}", fn_call, d)?;
                                continue;
                            }
                            writeln!(out, "{}", fn_call)?;
                        }
                        writeln!(out, "{: <12} {}", "", d)?;
                    }
                }
            }
            writeln!(out)?;
        }
        if let Some(examples) = self.examples() {
            let mut ex = "Example".to_string();
            if examples.len() > 1 {
                ex.push('s');
            }
            writeln!(out, "{}", ex)?;
            writeln!(out, "{1:-<0$}", ex.len(), "")?;
            for &(desc, example) in examples {
                let mut desc = desc.to_string();
                desc.push(':');
                for d in textwrap::wrap(&desc, 80) {
                    writeln!(out, "{}", d)?;
                }
                writeln!(out, "> {}", example)?;
                writeln!(out)?;
            }
        }
        Ok(out)
    }

    fn _format_md(&self) -> CliResult<String> {
        let mut out = String::with_capacity(10000);
        writeln!(out, "## {}", self.name())?;
        if let Some(desc) = self.desc() {
            writeln!(out, "{}\n", desc)?;
        }
        if self.vars().iter().any(|v| !v.hidden) {
            writeln!(out, "| variable/function | description |")?;
            writeln!(out, "|----|----|")?;
            for info in self.vars() {
                if !info.hidden {
                    writeln!(
                        out,
                        "| {} | {} |",
                        info.display_func().join(" or "),
                        info.description
                    )?;
                }
            }
        }
        if let Some(examples) = self.examples() {
            let mut ex = "Example".to_string();
            if examples.len() > 1 {
                ex.push('s');
            }
            writeln!(out, "### {}", ex)?;
            for &(desc, example) in examples {
                writeln!(out, "{}:", desc)?;
                writeln!(out, "```sh\n{}\n```", example)?;
            }
        }
        Ok(out)
    }
}

impl VarProviderInfo for Box<dyn VarProviderInfo> {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn desc(&self) -> Option<&'static str> {
        (**self).desc()
    }

    fn vars(&self) -> &[VarInfo] {
        (**self).vars()
    }

    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        (**self).examples()
    }
}

pub fn init_vars(
    modules: &mut Vec<Box<dyn VarProvider>>,
    custom_mod: Option<Box<dyn VarProvider>>,
    opts: &VarOpts,
    out_opts: &OutputOptions,
) -> CliResult<()> {
    // the custom module needs to be inserted early
    // at least before the expression module to make sure that
    // its variables are available in expressions
    if let Some(m) = custom_mod {
        modules.push(m);
    }

    // metadata lists
    modules.push(Box::new(
        modules::meta::MetaVars::new(
            &opts.metadata_sources,
            opts.meta_delim_override,
            opts.meta_dup_ids,
        )?
        .set_id_col(opts.meta_id_col)
        .set_has_header(opts.has_header),
    ));

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

pub fn get_var_help(
    custom_help: Option<Box<dyn VarProviderInfo>>,
    markdown: bool,
    command_only: bool,
) -> Result<String, fmt::Error> {
    let mut out = "".to_string();
    if let Some(m) = custom_help {
        writeln!(&mut out, "{}", m.format(markdown))?;
    }
    if !command_only {
        writeln!(
            &mut out,
            "{}",
            modules::builtins::BuiltinHelp.format(markdown)
        )?;
        writeln!(&mut out, "{}", modules::stats::StatHelp.format(markdown))?;
        writeln!(&mut out, "{}", modules::attr::AttrInfo.format(markdown))?;
        writeln!(&mut out, "{}", modules::meta::MetaInfo.format(markdown))?;
        #[cfg(feature = "expr")]
        writeln!(&mut out, "{}", modules::expr::ExprInfo.format(markdown))?;
    }
    Ok(out)
}
