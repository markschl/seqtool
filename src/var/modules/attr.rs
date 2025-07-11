use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::helpers::NA;
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
    /// Count the number of sequences for each unique value of an 'abund' attribute
    /// in the FASTA headers (.e.g. `>id abund=3`), which could be the number of
    /// duplicates obtained by the *unique* command (see `st unique --help-vars`)
    ///
    /// `st count -k 'attr(abund)' seqs.fa`
    ///
    /// 1	12019
    /// 2	2983
    /// 3	568
    /// (...)
    ///
    ///
    /// Summarize over a 'abund' attribute directly appended to the sequence ID
    /// like this `>id;abund=3`
    ///
    /// `st count -k 'attr(abund)' --attr-fmt ';key=value' seqs.fa`
    ///
    ///
    /// Summarize over an attribute 'a', which may be 'undefined' (=missing) in some
    /// headers
    ///
    /// `st count -k 'opt_attr(a)' seqs.fa`
    ///
    /// value1	6042
    /// value2	1012
    /// undefined	9566
    AttrVar {
        /// Obtain an attribute of given name (must be present in all sequences)
        Attr(Text) { name: String },
        /// Obtain an attribute value, or 'undefined' if missing
        /// (=undefined in JavaScript expressions)
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
        attrs: &Attributes,
        _: &mut QualConverter,
    ) -> Result<(), String> {
        for (symbol_id, var) in self.vars.iter() {
            let sym = symbols.get_mut(*symbol_id);
            match var.return_type {
                AttrVarType::Value => {
                    let opt_value = attrs.get_value(var.attr_id, rec.id(), rec.desc());
                    if let Some(val) = opt_value {
                        if val != NA.as_bytes() {
                            sym.inner_mut().set_text(val);
                            continue;
                        }
                        if !var.allow_missing {
                            return fail!(
                                "The value for attribute '{attr}' is '{na}', which is reserved for missing values. \
                                If '{na}' is meant to represent a missing value, use `opt_attr()` or `opt_attr_del()` \
                                to avoid this error. Otherwise, consider adjusting the attribute values to avoid '{na}'.",
                                attr=var.name, na=NA
                            );
                        }
                    }
                    if !var.allow_missing {
                        return fail!(
                            "Attribute '{}' not found in record '{}'. \
                            Use `opt_attr()` or `opt_attr_del()` if attributes are missing in some records. \
                            Use --attr-format to adjust the attribute key/value format.",
                            var.name,
                            String::from_utf8_lossy(rec.id())
                        );
                    }
                    sym.set_none();
                }
                AttrVarType::Exists => {
                    sym.inner_mut()
                        .set_bool(attrs.has_value(var.attr_id, rec.id(), rec.desc()));
                }
            }
        }
        Ok(())
    }
}
