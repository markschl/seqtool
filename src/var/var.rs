extern crate textwrap;

use std::fmt::{Debug, Write};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::clone::Clone;

use io::{Attribute, Record};
use io::input::InputOptions;
use io::output::OutputOptions;
use error::CliResult;

use super::symbols::Table;
use super::attr;

pub trait VarProvider: Debug + Send {
    fn prefix(&self) -> Option<&str>;
    fn name(&self) -> &'static str;
    fn register_var(&mut self, name: &str, id: usize, vars: &mut VarStore) -> CliResult<bool>;
    fn has_vars(&self) -> bool;
    fn set(&mut self, _: &Record, _: &mut Data) -> CliResult<()> {
        Ok(())
    }
    fn new_input(&mut self, _: &InputOptions) -> CliResult<()> {
        Ok(())
    }
    fn out_opts(&mut self, _: &OutputOptions) -> CliResult<()> {
        Ok(())
    }
}

pub trait VarHelp {
    fn name(&self) -> &'static str;
    fn usage(&self) -> &'static str;
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
        writeln!(out, "{}. Usage: {}", self.name(), self.usage()).unwrap();
        let w = self.name().len() + 9 + self.usage().len();
        writeln!(out, "{1:-<0$}", w, "").unwrap();
        if let Some(desc) = self.desc() {
            for d in textwrap::wrap_iter(desc, 80) {
                writeln!(out, "{}", d).unwrap();
            }
            writeln!(out, "").unwrap();
        }
        if let Some(v) = self.vars() {
            for &(name, desc) in v {
                for (i, d) in textwrap::wrap_iter(desc, 68).enumerate() {
                    let n = if i == 0 { name } else { "" };
                    writeln!(out, "{: <12} {}", n, d).unwrap();
                }
            }
            writeln!(out, "").unwrap();
        }
        if let Some(examples) = self.examples() {
            writeln!(out, "Example{}:", if examples.len() > 1 { "s" } else { "" }).unwrap();
            for &(desc, example) in examples {
                let mut desc = desc.to_string();
                desc.push_str(":");
                for d in textwrap::wrap_iter(&desc, 80) {
                    writeln!(out, "{}", d).unwrap();
                }
                writeln!(out, "> {}", example).unwrap();
            }
        }
        writeln!(out, "").unwrap();
        out
    }
}

impl<'a> VarProvider for Box<VarProvider + 'a> {
    fn prefix(&self) -> Option<&str> {
        (**self).prefix()
    }
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn register_var(&mut self, name: &str, id: usize, vars: &mut VarStore) -> CliResult<bool> {
        (**self).register_var(name, id, vars)
    }
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn set<'b>(&mut self, item: &'b Record, data: &mut Data) -> CliResult<()> {
        (**self).set(item, data)
    }
    fn new_input(&mut self, o: &InputOptions) -> CliResult<()> {
        (**self).new_input(o)
    }
    fn out_opts(&mut self, o: &OutputOptions) -> CliResult<()> {
        (**self).out_opts(o)
    }
}

impl<'a> VarProvider for &'a mut VarProvider {
    fn prefix(&self) -> Option<&str> {
        (**self).prefix()
    }
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn register_var(&mut self, name: &str, id: usize, vars: &mut VarStore) -> CliResult<bool> {
        (**self).register_var(name, id, vars)
    }
    fn has_vars(&self) -> bool {
        (**self).has_vars()
    }
    fn set<'b>(&mut self, item: &'b Record, data: &mut Data) -> CliResult<()> {
        (**self).set(item, data)
    }
    fn new_input(&mut self, o: &InputOptions) -> CliResult<()> {
        (**self).new_input(o)
    }
    fn out_opts(&mut self, o: &OutputOptions) -> CliResult<()> {
        (**self).out_opts(o)
    }
}

#[derive(Debug)]
pub struct Data {
    pub symbols: Table,
    pub attrs: attr::Attrs,
}

#[derive(Debug)]
pub struct Vars<'a> {
    varstore: VarStore,
    modules: Vec<Box<VarProvider + 'a>>,
    used_modules: Vec<usize>,
    data: Data,
}

impl<'a> Vars<'a> {
    pub fn new(attr_delim: u8, attr_value_delim: u8, attr_append_attr: Attribute) -> Vars<'a> {
        Vars {
            varstore: VarStore::new(),
            used_modules: vec![],
            modules: vec![],
            data: Data {
                symbols: Table::new(0),
                attrs: attr::Attrs::new(attr_delim, attr_value_delim, attr_append_attr),
            },
        }
    }

    pub fn build<F, O>(&mut self, action: F) -> CliResult<O>
    where
        F: FnMut(&mut VarBuilder) -> CliResult<O>,
    {
        self.build_with(None, action)
    }

    pub fn build_with<F, O>(
        &mut self,
        opt_mod: Option<&mut VarProvider>,
        mut action: F,
    ) -> CliResult<O>
    where
        F: FnMut(&mut VarBuilder) -> CliResult<O>,
    {
        let rv = {
            let mut builder = VarBuilder::new(&mut self.varstore);
            builder.add_modules(&mut self.modules);
            opt_mod.map(|m| builder.add_module(m));
            let rv = action(&mut builder);
            builder.done(&mut self.data);
            rv
        };
        self.find_used_modules();
        rv
    }

    fn find_used_modules(&mut self) {
        self.used_modules = self.modules
            .iter()
            .enumerate()
            .filter_map(|(i, m)| if m.has_vars() { Some(i) } else { None })
            .collect();
    }

    pub fn add_module<M>(&mut self, m: M)
    where
        M: VarProvider + 'a,
    {
        self.modules.push(Box::new(m));
    }

    pub fn parse_attrs(&mut self, rec: &Record) -> CliResult<()> {
        if self.data.attrs.has_attrs() {
            let (id, desc) = rec.id_desc_bytes();
            self.data.attrs.parse(id, desc);
        }
        Ok(())
    }

    pub fn new_input(&mut self, in_opts: &InputOptions) -> CliResult<()> {
        for &i in &self.used_modules {
            self.modules[i].new_input(in_opts)?;
        }
        Ok(())
    }

    pub fn out_opts(&mut self, o: &OutputOptions) -> CliResult<()> {
        for m in &mut self.modules {
            m.out_opts(o)?;
        }
        Ok(())
    }

    #[inline]
    pub fn set_record(&mut self, record: &Record) -> CliResult<()> {
        self.parse_attrs(record)?;

        for i in &self.used_modules {
            self.modules[*i].set(record, &mut self.data)?;
        }
        Ok(())
    }

    #[inline]
    pub fn symbols(&self) -> &Table {
        &self.data.symbols
    }

    #[inline]
    pub fn attrs(&self) -> &attr::Attrs {
        &self.data.attrs
    }

    #[inline]
    pub fn mut_data(&mut self) -> &mut Data {
        &mut self.data
    }
}

#[derive(Debug)]
pub struct VarBuilder<'a, 'b> {
    varstore: &'b mut VarStore,
    modules: HashMap<Option<String>, &'a mut VarProvider>,
}

impl<'a, 'b> VarBuilder<'a, 'b> {
    fn new(store: &'b mut VarStore) -> VarBuilder<'a, 'b> {
        VarBuilder {
            varstore: store,
            modules: HashMap::new(),
        }
    }

    fn add_modules<V, M>(&mut self, modules: M)
    where
        V: VarProvider + 'a,
        M: IntoIterator<Item = &'a mut V>,
    {
        for module in modules {
            self.add_module(module);
        }
    }

    fn add_module(&mut self, module: &'a mut VarProvider) -> bool {
        if let Entry::Vacant(e) = self.modules.entry(module.prefix().map(|s| s.to_string())) {
            e.insert(module);
            false
        } else {
            true
        }
    }

    pub fn register_attr(&mut self, name: &str, action: Option<attr::Action>) -> usize {
        self.varstore.register_attr(name, action)
    }

    pub fn register_var(&mut self, name: &str) -> CliResult<usize> {
        let (prefix, name) = split_name(name);
        self.register_with_prefix(prefix, name)
    }

    pub fn register_with_prefix(&mut self, prefix: Option<&str>, name: &str) -> CliResult<usize> {

        let key = (prefix.map(|s| s.to_string()), name.to_string());

        let (id, exists) = self.varstore.register_with_prefix(prefix, name);
        if exists {
            return Ok(id);
        }

        self.mod_register(&key.0, name, id).map_err(|e| {
            self.varstore.remove_last_var();
            e
        })?;

        self.varstore.var_validated(&key, true);

        for (key, id) in self.varstore.get_new_vars() {
            self.mod_register(&key.0, &key.1, id).map_err(|e| {
                self.varstore.remove_last_var();
                e
            })?;
            self.varstore.var_validated(&key, true);
        }

        Ok(id)
    }

    // searches for correct module given <prefix> and registers a variable with <name> and <id> to it
    fn mod_register(&mut self, prefix: &Option<String>, name: &str, id: usize) -> CliResult<()> {
        if let Some(module) = self.modules.get_mut(prefix) {
            let found = module.register_var(name, id, self.varstore)?;
            if !found {
                return fail!(format!("Unknown {} variable: '{}'.", module.name(), name));
            }
            Ok(())
        } else {
            fail!(format!(
                "Unknown variable prefix: {}",
                prefix.as_ref().map(|s| s.as_str()).unwrap_or("(builtin)")
            ))
        }
    }

    // must be called before destruction
    fn done(self, data: &mut Data) {
        self.varstore.reg_attrs(data);
    }
}

fn split_name(name: &str) -> (Option<&str>, &str) {
    let s: Vec<_> = name.splitn(2, ':').collect();
    if s.len() == 2 {
        (Some(s[0]), s[1])
    } else {
        (None, s[0])
    }
}

#[derive(Debug, Clone)]
pub struct VarStore {
    num_vars: usize,
    num_attrs: usize,
    // K: (prefix, name), V: (id, registered?)
    vars: HashMap<(Option<String>, String), (usize, bool)>,
    // K: name, V: (id, action, registered?)
    attrs: HashMap<String, (usize, Option<attr::Action>, bool)>,
}

impl VarStore {
    fn new() -> VarStore {
        VarStore {
            num_vars: 0,
            num_attrs: 0,
            vars: HashMap::new(),
            attrs: HashMap::new(),
        }
    }

    pub fn register_var(&mut self, name: &str) -> (usize, bool) {
        let (prefix, name) = split_name(name);
        self.register_with_prefix(prefix, name)
    }

    pub fn register_with_prefix(&mut self, prefix: Option<&str>, name: &str) -> (usize, bool) {
        let key = (prefix.map(|s| s.to_string()), name.to_string());

        if let Some(&(id, _)) = self.vars.get(&key) {
            return (id, true);
        }

        let id = self.num_vars;
        self.num_vars += 1;
        self.vars.insert(key, (id, false));
        (id, false)
    }

    pub fn register_attr(&mut self, name: &str, action: Option<attr::Action>) -> usize {
        if let Some(&(id, _, _)) = self.attrs.get(name) {
            return id;
        }
        let id = self.num_attrs;
        self.num_attrs += 1;
        self.attrs.insert(name.to_string(), (id, action, false));
        id
    }

    fn var_validated(&mut self, key: &(Option<String>, String), flag: bool) {
        self.vars.get_mut(key).map(|i| i.1 = flag);
    }

    fn get_new_vars(&self) -> Vec<((Option<String>, String), usize)> {
        self.vars
            .iter()
            .filter_map(|(key, &(id, registered))| {
                if !registered {
                    Some((key.clone(), id))
                } else {
                    None
                }
            })
            .collect()
    }

    // allows recovery after error
    fn remove_last_var(&mut self) {
        if self.num_vars > 0 {
            self.num_vars -= 1;
            self.vars = self.vars
                .iter()
                .filter_map(|(k, v)| {
                    if v.0 != self.num_vars {
                        Some((k.clone(), *v))
                    } else {
                        None
                    }
                })
                .collect();
        }
    }

    // must be called before destruction
    fn reg_attrs(&mut self, data: &mut Data) {
        data.symbols.resize(self.num_vars);

        let mut p: Vec<_> = self.attrs.iter_mut().collect();
        p.sort_by_key(|&(_, &mut (id, _, _))| id);
        for (name, &mut (id, action, ref mut registered)) in p {
            if !*registered {
                data.attrs.add_attr(name, id, action);
                *registered = true;
            }
        }
    }
}
