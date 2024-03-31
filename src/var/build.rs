use std::clone::Clone;

use crate::error::{CliError, CliResult};
use crate::helpers::DefaultHashMap as HashMap;

use super::attr::{AttrWriteAction, Attributes};
use super::func::Func;
use super::symbols::VarType;
use super::VarProvider;

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
    // varname -> (module_idx, (min_args, max_args))
    var_map: &'a HashMap<String, (usize, (usize, usize))>,
    // func -> (var_id, var_type, allow_nested)
    registered_vars: &'a mut HashMap<Func, (usize, Option<VarType>, bool)>,
    attrs: &'a mut Attributes,
}

impl<'a> VarBuilder<'a> {
    pub fn new(
        modules: &'a mut [Box<dyn VarProvider>],
        var_map: &'a HashMap<String, (usize, (usize, usize))>,
        registered_vars: &'a mut HashMap<Func, (usize, Option<VarType>, bool)>,
        attrs: &'a mut Attributes,
    ) -> Self {
        Self {
            modules,
            var_map,
            registered_vars,
            attrs,
        }
    }

    pub fn register_attr(
        &mut self,
        name: &str,
        action: Option<AttrWriteAction>,
    ) -> Result<Option<usize>, String> {
        self.attrs.add_attr(name, action)
    }

    /// Attempts at registering a variable/function
    /// Returns `Some((var_id, var_type, allow_nested))` if successful
    pub fn register_var(
        &mut self,
        var: &Func,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        self._register_var(var, false)
    }

    /// Register a variable/function from within another `VarProvider`
    pub fn register_nested_var(
        &mut self,
        var: &Func,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        self._register_var(var, true)
    }

    pub fn _register_var(
        &mut self,
        func: &Func,
        nested: bool,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        if let Some((id, var_type, allow_nested)) = self.registered_vars.get(func) {
            // eprintln!("var present {:?} {} {:?}", func, id, var_type);
            if nested && !allow_nested {
                return Err(NestedVarError(func.name.to_string()).into());
            }
            return Ok(Some((*id, var_type.clone(), true)));
        }
        if let Some(mod_idx) = self.lookup_var(func).transpose()? {
            let (dep_mod, other_mod) = self.modules.split_at_mut(mod_idx);
            let var_mod = other_mod.first_mut().unwrap();
            // dbg!("=============");
            // dbg!(var_mod.info());
            // dbg!("=============");
            // for m in &*dep_mod {
            //     dbg!(m.info());
            // }
            let mut nested_builder = VarBuilder {
                modules: dep_mod,
                var_map: self.var_map,
                registered_vars: self.registered_vars,
                attrs: self.attrs,
            };
            let vtype = var_mod.register(func, &mut nested_builder)?;
            let allow_nested = var_mod.allow_nested();
            if nested && !allow_nested {
                return Err(NestedVarError(func.name.to_string()).into());
            }
            let var_id = self.registered_vars.len();
            self.registered_vars
                .insert(func.clone(), (var_id, vtype.clone(), allow_nested));
            // eprintln!(
            //     "successful {:?}  =>  {} / {:?} in  {:?}",
            //     func, var_id, vtype, var_mod
            // );
            return Ok(Some((var_id, vtype, false)));
        }
        Ok(None)
    }

    pub fn has_var(&self, varname: &str) -> bool {
        self.var_map
            .get(varname)
            .map(|(i, _)| *i < self.modules.len())
            .unwrap_or(false)
    }

    // returns the module index of the given function/variable if known
    fn lookup_var(&'a self, func: &Func) -> Option<Result<usize, String>> {
        self.var_map
            .get(&func.name)
            .and_then(|(i, (min_args, max_args))| {
                // dbg!(&func, i, min_args, max_args, self.modules.len());
                if *i < self.modules.len() {
                    // validate function args
                    let num_args = func.args.len();
                    let what = if num_args < *min_args {
                        "Not enough"
                    } else if num_args > *max_args {
                        "Too many"
                    } else {
                        return Some(Ok(*i));
                    };
                    Some(Err(format!(
                        "{} arguments provided to function '{}', expected {} but found {}.",
                        what,
                        func.name,
                        if min_args != max_args {
                            format!("{}-{}", min_args, max_args)
                        } else {
                            min_args.to_string()
                        },
                        num_args
                    )))
                } else {
                    None
                }
            })
    }

    /// Current variable ID, which `VarProvider`s may store for later accessing values from the symbol
    /// table.
    pub fn symbol_id(&self) -> usize {
        self.registered_vars.len()
    }
}

pub struct NestedVarError(String);

impl From<NestedVarError> for CliError {
    fn from(e: NestedVarError) -> Self {
        CliError::Other(format!(
            "The variable/function '{}' can unfortunately not be used as \
            within an {{ expression }}.",
            e.0
        ))
    }
}
