use std::clone::Clone;

use fxhash::FxHashMap;

use crate::error::{CliError, CliResult};

use super::attr::{Action, Attrs};
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
/// However, it *must* use `VarBuilder::register_dependent_var()`, since vars/functions
/// from certain providers used in commands (that have `VarProvider::allow_dependent() == false`)
/// cannot be used in expressions.
#[derive(Debug)]
pub struct VarBuilder<'a> {
    modules: &'a mut [Box<dyn VarProvider>],
    // func -> (var_id, var_type, allow_nested)
    var_map: &'a mut FxHashMap<Func, (usize, Option<VarType>, bool)>,
    attr_map: &'a mut FxHashMap<String, usize>,
    attrs: &'a mut Attrs,
}

impl<'a> VarBuilder<'a> {
    pub fn new(
        modules: &'a mut [Box<dyn VarProvider>],
        var_map: &'a mut FxHashMap<Func, (usize, Option<VarType>, bool)>,
        attr_map: &'a mut FxHashMap<String, usize>,
        attrs: &'a mut Attrs,
    ) -> Self {
        Self {
            modules,
            var_map,
            attr_map,
            attrs,
        }
    }

    pub fn register_attr(&mut self, name: &str, action: Option<Action>) -> usize {
        if let Some(&attr_id) = self.attr_map.get(name) {
            return attr_id;
        }
        let attr_id = self.attr_map.len();
        self.attr_map.insert(name.to_string(), attr_id);
        self.attrs.add_attr(name, attr_id, action);
        attr_id
    }

    /// Attempts at registering a variable/function
    /// Returns `Some((var_id, var_type, allow_nested))` if successful
    pub fn register_var(
        &mut self,
        var: &Func,
    ) -> CliResult<Option<(usize, Option<VarType>, bool)>> {
        self._register_var(var, false)
    }

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
