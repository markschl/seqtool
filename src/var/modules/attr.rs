use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::io::{QualConverter, Record};
use crate::var::{
    attr::{self, Attributes},
    parser::Arg,
    symbols::SymbolTable,
    VarBuilder, VarStore,
};

use super::VarProvider;

variable_enum! {
    /// # Header attributes
    ///
    /// Attributes stored in FASTA/FASTQ headers.
    /// The expected pattern is ' key=value', but other patterns can be
    /// specified with `--attr-format`.
    ///
    /// # Examples
    ///
    /// Summarizing over an attribute in the FASTA header `>id size=3`
    ///
    /// `st count -k 'attr(size)' seqs.fa`
    ///
    /// Summarizing over a 'size' attribute that may be missing/empty in some
    /// headers
    ///
    /// `st count -k 'opt_attr(size)' seqs.fa`
    ///
    /// Summarize over a 'size' attribute directly appended to the sequence ID
    /// like this: '>id;size=3 description
    ///
    /// `st count -k 'opt_attr(size)' --attr-fmt ';key=value' seqs.fa`
    ///
    AttrVar {
        /// Obtain an attribute of given name (must be present in all sequences)
        Attr(Text) { name: String },
        /// Obtain an attribute value, or an empty string if missing. In
        /// Javascript expressions, missing attributes equal to `undefined`.
        OptAttr(Text) { name: String },
        /// Obtain an attribute (must be present), simultaneously removing
        /// it from the header.
        AttrDel(Text) { name: String },
        /// Obtain an attribute (may be missing), simultaneously removing
        /// it from the header.
        OptAttrDel(Text) { name: String },
        /// Returns `true` if the given attribute is present, otherwise
        /// returns `false`. Especially useful with the `filter` command;
        /// equivalent to the expression `opt_attr(name) != undefined`.
        HasAttr(Boolean) { name: String },
    }
}

/// A registered variable type
#[derive(Debug, Clone, PartialEq)]
struct RegAttrVar {
    attr_id: usize,
    name: String,
    return_type: AttrVarType,
    allow_missing: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum AttrVarType {
    Value,
    Exists,
}

#[derive(Debug, Default)]
pub struct AttrVars {
    vars: VarStore<RegAttrVar>,
}

impl AttrVars {
    pub fn new() -> AttrVars {
        Self::default()
    }
}

impl VarProvider for AttrVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(AttrVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        if let Some((var, out_type)) = AttrVar::from_func(name, args)? {
            use AttrVar::*;
            let (name, paction, rtype, allow_missing) = match var {
                Attr { name } => (name, None, AttrVarType::Value, false),
                OptAttr { name } => (name, None, AttrVarType::Value, true),
                AttrDel { name } => (
                    name,
                    Some(attr::AttrWriteAction::Delete),
                    AttrVarType::Value,
                    false,
                ),
                OptAttrDel { name } => (
                    name,
                    Some(attr::AttrWriteAction::Delete),
                    AttrVarType::Value,
                    true,
                ),
                HasAttr { name } => (name, None, AttrVarType::Exists, true),
            };
            if name.is_empty() {
                return fail!("Attribute names cannot be empty.");
            }
            let attr_id = builder.register_attr(&name, paction)?.unwrap();
            let reg_var = RegAttrVar {
                name: name.to_string(),
                return_type: rtype,
                attr_id,
                allow_missing,
            };
            let symbol_id = builder.store_register(reg_var, &mut self.vars);
            return Ok(Some((symbol_id, out_type)));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set_record(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        attrs: &mut Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        for (symbol_id, var) in self.vars.iter() {
            let sym = symbols.get_mut(*symbol_id);
            match var.return_type {
                AttrVarType::Value => {
                    if let Some(val) = attrs.get_value(var.attr_id, rec.id(), rec.desc()) {
                        sym.inner_mut().set_text(val);
                    } else {
                        if !var.allow_missing {
                            return fail!(format!(
                                "Attribute '{}' not found in record '{}'. \
                                Use 'opt_attr()' if attributes are missing in some records. \
                                Use --attr-format to adjust the attribute key/value format.",
                                var.name,
                                String::from_utf8_lossy(rec.id())
                            ));
                        }
                        sym.set_none();
                    }
                }
                AttrVarType::Exists => {
                    sym.inner_mut().set_bool(attrs.has_value(var.attr_id));
                }
            }
        }
        Ok(())
    }
}
