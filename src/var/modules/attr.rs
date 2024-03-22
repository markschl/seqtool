use crate::error::CliResult;
use crate::io::{QualConverter, Record};
use crate::var::VarInfo;
use crate::var::{
    attr::{self, Attributes},
    func::Func,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct AttrInfo;

impl VarProviderInfo for AttrInfo {
    fn name(&self) -> &'static str {
        "Header attributes"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(
                attr ( name ) =>
                "Obtain an attribute of given name (must be present in all sequences)"
            ),
            var_info!(
                opt_attr ( name ) =>
                "Obtain an attribute value, or an empty string if missing. In \
                Javascript expressions, missing attributes equal to `undefined`."
            ),
            var_info!(
                attr_del ( name ) =>
                "Obtain an attribute (must be present), simultaneously removing \
                it from the header."
            ),
            var_info!(
                opt_attr_del ( name ) =>
                "Obtain an attribute (may be missing), simultaneously removing \
                it from the header."
            ),
            var_info!(
                has_attr ( name ) =>
                "Returns `true` if the given attribute is present, otherwise \
                returns `false`. Especially useful with the `filter` command; \
                equivalent to the expression `opt_attr(name) != undefined`."
            ),
        ]
    }

    fn desc(&self) -> Option<&'static str> {
        Some("Attributes stored in FASTA/FASTQ headers in the form key=value")
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Summarizing over an attribute in the FASTA header `>id size=3`",
                "st count -k attr(size) seqs.fa",
            ),
            (
                "Adding the sequence length to the header as attribute",
                "st . -a seqlen={seqlen} seqs.fa",
            ),
        ])
    }
}

#[derive(Debug)]
enum AttrVarType {
    Value,
    Exists,
}

#[derive(Debug)]
struct Var {
    name: String,
    return_type: AttrVarType,
    attr_id: usize,
    id: usize,
    allow_missing: bool,
}

#[derive(Debug, Default)]
pub struct AttrVars {
    vars: Vec<Var>,
}

impl AttrVars {
    pub fn new() -> AttrVars {
        Self::default()
    }
}

impl VarProvider for AttrVars {
    fn info(&self) -> &dyn VarProviderInfo {
        &AttrInfo
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let (paction, rtype, vtype, allow_missing) = match func.name.as_str() {
            "attr" => (None, AttrVarType::Value, VarType::Text, false),
            "has_attr" => (None, AttrVarType::Exists, VarType::Bool, true),
            "opt_attr" => (None, AttrVarType::Value, VarType::Text, true),
            "attr_del" => (
                Some(attr::AttrWriteAction::Delete),
                AttrVarType::Value,
                VarType::Text,
                false,
            ),
            "opt_attr_del" => (
                Some(attr::AttrWriteAction::Delete),
                AttrVarType::Value,
                VarType::Text,
                true,
            ),
            _ => unreachable!(), // shouldn't happen
        };
        let name = func.arg_as::<String>(0)?;
        if name.is_empty() {
            return fail!("Attribute names cannot be empty.");
        }
        let attr_id = b.register_attr(&name, paction)?.unwrap();
        self.vars.push(Var {
            name: name.to_string(),
            return_type: rtype,
            attr_id,
            id: b.symbol_id(),
            allow_missing,
        });
        Ok(Some(vtype))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        attrs: &mut Attributes,
        _: &mut QualConverter,
    ) -> CliResult<()> {
        for var in &self.vars {
            let sym = symbols.get_mut(var.id);
            match var.return_type {
                AttrVarType::Value => {
                    if let Some(val) = attrs.get_value(var.attr_id, rec.id(), rec.desc()) {
                        sym.inner_mut().set_text(val);
                    } else {
                        if !var.allow_missing {
                            return fail!(format!(
                                "Attribute '{}' not found in record '{}'. \
                                Use 'opt_attr()' if attributes may be missing in some records. \
                                Set the correct attribute format with --attr-format.",
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
