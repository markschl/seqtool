use std::clone::Clone;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use var_provider::VarType;

use super::attr::{AttrWriteAction, Attributes};
use super::modules::{expr::js::parser::Expression, VarProvider};
use super::parser::Arg;

/// Object used for registering variables/functions to different `VarProvider` modules.
/// It does so by 'asking' each of the providers (in order of occurrence),
/// whether it 'knows' a function name. If so, the provider will be presented
/// with each sequence record (using `VarProvider::set()`) and is then responsible
/// for updating the `SymbolTable` with values for all the variables that it knows.
///
/// A `VarProvider` can itself register variables, querying all providers *before*
/// it in the modules list. This functionality is used by `expr::ExprVars`.
/// However, it *must* use `VarBuilder::register_nested_var()`, since vars/functions
/// from certain providers used in commands (that have `VarProvider::allow_nested() == false`)
/// cannot be used in expressions.
#[derive(Debug)]
pub struct VarBuilder<'a> {
    modules: &'a mut [Box<dyn VarProvider>],
    attrs: &'a mut Attributes,
    num_symbols: &'a mut usize,
}

impl<'a> VarBuilder<'a> {
    pub fn new(
        modules: &'a mut [Box<dyn VarProvider>],
        attrs: &'a mut Attributes,
        num_symbols: &'a mut usize,
    ) -> Self {
        Self {
            modules,
            attrs,
            num_symbols,
        }
    }

    /// Attempts at registering a header attribute with the given action
    /// (read, edit, append, delete).
    ///
    /// Returns an attribute ID, which can be used to later access the current value
    /// of the attribute using `Attributes::get_value()` within the
    /// `VarProvider::set_record()` method.
    ///
    /// Returns an error if incompatibility with earlier attributes was found
    /// (contradicting actions).
    ///
    /// Returns `None` only with `AttrWriteAction::Append`, since this action
    /// means that header attributes are not parsed at all, only written.
    pub fn register_attr(
        &mut self,
        name: &str,
        action: Option<AttrWriteAction>,
    ) -> Result<Option<usize>, String> {
        self.attrs.add_attr(name, action)
    }

    /// Attempts at registering a variable/function
    /// Returns `Ok(Some((var_id, output type)))` if found and valid,
    /// returns an error if the arguments are invalid.
    pub fn register_var(
        &mut self,
        name: &str,
        args: &[Arg],
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        // Try registering the variable to the *last* module in the list
        if let Some((last_mod, other)) = self.modules.split_last_mut() {
            // Create another builder, which holds all modules *preceding* the
            // last module in the list.
            // In `VarProvider::set_record()`, these modules will be invoked first,
            // so all modules later in the list will have access to variables of the
            // earlier modules (but not later modules).
            let mut nested_builder = VarBuilder {
                modules: other,
                attrs: self.attrs,
                num_symbols: self.num_symbols,
            };
            // this is a recursive call (until there are no more modules to process)
            let out = last_mod.register(name, args, &mut nested_builder)?;
            if out.is_some() {
                return Ok(out);
            }
            // if the variable was not found, try the next module by recursively
            // calling the current function
            return nested_builder.register_var(name, args);
        }
        Ok(None)
    }

    /// Returns whether the given variable name exists in any of the modules
    pub fn has_var(&self, name: &str) -> bool {
        self.modules
            .iter()
            .flat_map(|m| m.info().vars().iter().map(|v| v.name))
            .any(|n| n == name)
    }

    /// Returns a new symbol ID, which can be assigned to some variable.
    /// The calling variable provider is responsible for storing it and then
    /// assigning a value to the given slot in the symbol table in
    /// `VarProvider::set_record()`.
    pub fn increment(&mut self) -> usize {
        let id = *self.num_symbols;
        *self.num_symbols += 1;
        id
    }

    /// A function called by individual `VarProvider` modules that internally
    /// use a `VarStore` to finalize the variable registration.
    /// A new symbol ID is assigned to the variable, which is added to the
    /// variable store.
    pub fn store_register<V>(&mut self, var: V, out: &mut VarStore<V>) -> usize
    where
        V: PartialEq + Clone,
    {
        if let Some(id) = out.lookup(&var) {
            return id;
        }
        let id = self.increment();
        out.add(id, var);
        id
    }

    /// Registers a JS expression, assuming that an expression engine
    /// is present in the list of variable providers.
    /// Panics otherwise.
    #[inline]
    pub fn register_expr(&mut self, expr: &Expression) -> Result<(usize, Option<VarType>), String> {
        Ok(self
            .register_var("_____expr", &[Arg::Expr(expr.clone())])?
            .unwrap())
    }
}

/// Very simple wrapper around a vector of (symbol ID, variable) pairs.
/// This is used by most variable providers internally to store and look up
/// variables using simple linear search.
#[derive(Debug)]
pub struct VarStore<V: PartialEq> {
    vars: Vec<(usize, V)>,
}

impl<V: PartialEq> Default for VarStore<V> {
    fn default() -> Self {
        Self { vars: Vec::new() }
    }
}

impl<V: PartialEq> VarStore<V> {
    fn add(&mut self, id: usize, var: V) {
        self.vars.push((id, var));
    }

    fn lookup(&self, var: &V) -> Option<usize> {
        self.vars
            .iter()
            .find_map(|(id, v)| if v == var { Some(*id) } else { None })
    }
}

impl<V: PartialEq> Deref for VarStore<V> {
    type Target = Vec<(usize, V)>;

    fn deref(&self) -> &Self::Target {
        &self.vars
    }
}

impl<V: PartialEq> DerefMut for VarStore<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vars
    }
}
