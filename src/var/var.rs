extern crate textwrap;

use std::any::Any;
use std::clone::Clone;
use std::collections::HashMap;
use std::fmt::{Debug, Write};

use crate::error::{CliError, CliResult};
use crate::io::{input::InputOptions, output::OutputOptions, QualConverter, Record, SeqAttr};

use super::attr;
use super::symbols::{SymbolTable, VarType};

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct Func {
    pub name: String,
    // there can be a maximum of 4 args
    pub args: Vec<String>,
}

impl Func {
    #[allow(dead_code)]
    pub fn var(name: String) -> Self {
        Self::with_args(name, Default::default())
    }

    pub fn expr(expr: &str) -> Self {
        Self::with_args("____expr".to_string(), vec![format!("'{}'", expr)])
    }

    pub fn with_args(name: String, args: Vec<String>) -> Self {
        Self { name, args }
    }

    pub fn num_args(&self) -> usize {
        self.args.len()
    }

    pub fn ensure_num_args(&self, num_args: usize) -> Result<(), String> {
        self.ensure_arg_range(num_args, num_args)
    }

    pub fn ensure_arg_range(&self, min_args: usize, max_args: usize) -> Result<(), String> {
        let n = self.num_args();
        // if n == 0 && max_args > 0 {
        //     return Err(format!("'{}' is not a function with arguments, but a simple variable", self.name));
        // }
        let what = if n < min_args {
            "Not enough"
        } else if n > max_args {
            "Too many"
        } else {
            return Ok(());
        };
        Err(format!(
            "{} arguments provided to function '{}', expected {} but found {}.",
            what,
            self.name,
            if min_args != max_args {
                format!("{}-{}", min_args, max_args)
            } else {
                min_args.to_string()
            },
            n
        ))
    }

    pub fn ensure_no_args(&self) -> Result<(), String> {
        self.ensure_num_args(0)
    }

    // pub fn one_arg(&self) -> Result<&str, String> {
    //     self.ensure_num_args(1)?;
    //     Ok(&self.args[0].as_ref())
    // }

    pub fn one_arg_as<T: ArgValue>(&self) -> Result<T, String> {
        self.ensure_num_args(1)?;
        self.arg_as(0).unwrap()
    }

    pub fn arg(&self, num: usize) -> Option<&str> {
        self.args.get(num).map(String::as_str)
    }

    pub fn arg_as<T: ArgValue>(&self, num: usize) -> Option<Result<T, String>> {
        self.arg(num).map(|a| {
            ArgValue::from_str(a)
                .ok_or_else(|| format!("Invalid argument for {}: {}", self.name, a))
        })
    }

    // pub fn str_arg_or_empty(&self, num: usize) -> Result<String, String> {
    //     self.arg_as(num).unwrap_or_else(|| Ok("".to_string()))
    // }
}

pub trait ArgValue {
    fn from_str(val: &str) -> Option<Self>
    where
        Self: Sized;
}

impl ArgValue for String {
    fn from_str(val: &str) -> Option<Self> {
        if let Some(&c0) = val.as_bytes().first() {
            if c0 == b'"' || c0 == b'\'' {
                let c1 = *val.as_bytes().last().unwrap();
                if c0 != c1 {
                    return None;
                }
                return Some(val[1..val.len() - 1].to_string());
            }
            // TODO: we currently allow non-quoted string arguments
            // (not valid javascript)
            return Some(val.to_string());
        }
        None
    }
}

impl ArgValue for i64 {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for usize {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for f64 {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for bool {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

pub trait AsAnyMut {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAnyMut for T {
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// TODO: move to var/modules/mod.rs
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
    fn set(&mut self, _: &dyn Record, _: &mut MetaData) -> CliResult<()> {
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
    fn set(&mut self, item: &'_ dyn Record, data: &mut MetaData) -> CliResult<()> {
        (**self).set(item, data)
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
    fn format(&self) -> String {
        let mut out = String::new();
        if let Some(u) = self.usage() {
            writeln!(out, "{}. Usage: {}", self.name(), u).unwrap();
            let w = self.name().len() + 9 + u.len().min(80);
            writeln!(out, "{1:-<0$}", w, "").unwrap();
        } else {
            writeln!(out, "{}\n{2:-<1$}", self.name(), self.name().len(), "").unwrap();
        }
        if let Some(desc) = self.desc() {
            for d in textwrap::wrap(desc, 80) {
                writeln!(out, "{}", d).unwrap();
            }
            writeln!(out).unwrap();
        }
        if let Some(v) = self.vars() {
            for &(name, desc) in v {
                for (i, d) in textwrap::wrap(desc, 68).into_iter().enumerate() {
                    let n = if i == 0 { name } else { "" };
                    writeln!(out, "{: <12} {}", n, d).unwrap();
                }
            }
            writeln!(out).unwrap();
        }
        if let Some(examples) = self.examples() {
            writeln!(out, "Example{}:", if examples.len() > 1 { "s" } else { "" }).unwrap();
            for &(desc, example) in examples {
                let mut desc = desc.to_string();
                desc.push(':');
                for d in textwrap::wrap(&desc, 80) {
                    writeln!(out, "{}", d).unwrap();
                }
                writeln!(out, "> {}", example).unwrap();
            }
        }
        writeln!(out).unwrap();
        out
    }
}

#[derive(Debug)]
pub struct MetaData {
    pub symbols: SymbolTable,
    pub attrs: attr::Attrs,
    pub qual_converter: QualConverter,
}

#[derive(Debug)]
pub struct Vars {
    modules: Vec<Box<dyn VarProvider>>,
    data: MetaData,
    var_map: HashMap<Func, (usize, Option<VarType>, bool)>,
    attr_map: HashMap<String, usize>,
    print_help: bool,
}

impl Vars {
    pub fn new(
        attr_delim: u8,
        attr_value_delim: u8,
        append_attr: SeqAttr,
        qual_converter: QualConverter,
        print_help: bool,
    ) -> Self {
        Vars {
            modules: vec![],
            data: MetaData {
                symbols: SymbolTable::new(0),
                attrs: attr::Attrs::new(attr_delim, attr_value_delim, append_attr),
                qual_converter,
            },
            var_map: HashMap::new(),
            attr_map: HashMap::new(),
            print_help,
        }
    }

    pub fn build<F, O>(&mut self, mut action: F) -> CliResult<O>
    where
        F: FnMut(&mut VarBuilder) -> CliResult<O>,
    {
        let rv = {
            let mut builder = VarBuilder {
                modules: &mut self.modules,
                attrs: &mut self.data.attrs,
                var_map: &mut self.var_map,
                attr_map: &mut self.attr_map,
            };
            action(&mut builder)
        };
        // done, grow the symbol table
        self.data.symbols.resize(self.var_map.len());
        rv
    }

    pub fn finalize(&mut self) {
        // print help if needed
        if self.print_help {
            eprintln!("{}", self.get_help());
            std::process::exit(2);
        }
        // remove unused modules
        self.modules = self.modules.drain(..).filter(|m| m.has_vars()).collect();
        // println!("vars final {:?}", self);
    }

    pub fn add_module(&mut self, m: Box<dyn VarProvider>) {
        self.modules.push(m);
    }

    pub fn custom_mod<M, O>(
        &mut self,
        func: impl FnOnce(Option<&mut M>, &mut SymbolTable) -> CliResult<O>,
    ) -> CliResult<O>
    where
        M: VarProvider + 'static,
    {
        let m = self.modules.first_mut().unwrap();
        let m = m.as_mut().as_any_mut().downcast_mut::<M>();
        func(m, &mut self.data.symbols)
    }

    pub fn parse_attrs(&mut self, rec: &dyn Record) -> CliResult<()> {
        if self.data.attrs.has_attrs() {
            let (id, desc) = rec.id_desc_bytes();
            self.data.attrs.parse(id, desc);
        }
        Ok(())
    }

    pub fn new_input(&mut self, in_opts: &InputOptions) -> CliResult<()> {
        for m in &mut self.modules {
            m.new_input(in_opts)?;
        }
        Ok(())
    }

    pub fn init_output(&mut self, o: &OutputOptions) -> CliResult<()> {
        for m in &mut self.modules {
            m.init(o)?;
        }
        Ok(())
    }

    #[inline]
    pub fn set_record(&mut self, record: &dyn Record) -> CliResult<()> {
        self.parse_attrs(record)?;
        for m in &mut self.modules {
            m.set(record, &mut self.data)?;
        }
        Ok(())
    }

    #[inline]
    pub fn symbols(&self) -> &SymbolTable {
        &self.data.symbols
    }

    // #[inline]
    // pub fn symbols_mut(&mut self) -> &mut SymbolTable {
    //     &mut self.data.symbols
    // }

    #[inline]
    pub fn attrs(&self) -> &attr::Attrs {
        &self.data.attrs
    }

    #[inline]
    pub fn data(&self) -> &MetaData {
        &self.data
    }

    // #[inline]
    // pub fn mut_data(&mut self) -> &mut MetaData {
    //     &mut self.data
    // }

    #[inline]
    pub fn get_help(&self) -> String {
        self.modules
            .iter()
            .map(|m| m.help().format())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug)]
pub struct VarBuilder<'a> {
    modules: &'a mut [Box<dyn VarProvider>],
    // func -> (var_id, var_type, allow_nested)
    var_map: &'a mut HashMap<Func, (usize, Option<VarType>, bool)>,
    attr_map: &'a mut HashMap<String, usize>,
    attrs: &'a mut attr::Attrs,
}

impl<'a> VarBuilder<'a> {
    pub fn register_attr(&mut self, name: &str, action: Option<attr::Action>) -> usize {
        if let Some(&attr_id) = self.attr_map.get(name) {
            return attr_id;
        }
        let attr_id = self.attr_map.len();
        self.attr_map.insert(name.to_string(), attr_id);
        self.attrs.add_attr(name, attr_id, action);
        attr_id
    }

    pub fn register_var(
        &mut self,
        var: &Func,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        self._register_var(var, false)
    }

    #[allow(dead_code)]
    pub fn register_dependent_var(
        &mut self,
        var: &Func,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        self._register_var(var, true)
    }

    pub fn _register_var(
        &mut self,
        var: &Func,
        dependent: bool,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        if let Some((id, var_type, allow_nested)) = self.var_map.get(var) {
            // eprintln!("var present {:?} {} {:?}", var, id, var_type);
            if dependent && !allow_nested {
                return Err(DependentVarError(var.name.to_string()).into());
            }
            return Ok(Some((*id, var_type.clone(), true)));
        }
        if let Some((t, other)) = self.modules.split_last_mut() {
            let mut b = VarBuilder {
                modules: other,
                attrs: self.attrs,
                var_map: self.var_map,
                attr_map: self.attr_map,
            };
            if let Some(vtype) = t.register(var, &mut b)? {
                let var_id = self.var_map.len();
                let allow_nested = t.allow_dependent();
                if dependent && !allow_nested {
                    return Err(DependentVarError(var.name.to_string()).into());
                }
                self.var_map
                    .insert(var.clone(), (var_id, vtype.clone(), allow_nested));
                // eprintln!("successful {:?}  =>  {} / {:?} in  {:?}", var, var_id, vtype, t);
                return Ok(Some((var_id, vtype, false)));
            }
            return b._register_var(var, dependent);
        }
        Ok(None)
    }

    pub fn symbol_id(&self) -> usize {
        self.var_map.len()
    }
}

pub struct DependentVarError(String);

impl From<DependentVarError> for CliError {
    fn from(e: DependentVarError) -> Self {
        CliError::Other(format!(
            "The variable/function '{}' can unfortunately not be used as \
            within an {{ expression }}.",
            e.0
        ))
    }
}
