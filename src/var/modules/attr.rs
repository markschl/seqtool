use crate::error::CliResult;
use crate::io::Record;
use crate::var::*;

pub struct AttrHelp;

impl VarHelp for AttrHelp {
    fn name(&self) -> &'static str {
        "Attributes"
    }
    fn usage(&self) -> Option<&'static str> {
        Some("attr(name) or attr('name') or attr(\"name\")")
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "attr(name) or attr('name') or attr(\"name\")",
                "Obtain an attribute of given name (must be present in all sequences)",
            ),
            (
                "opt_attr(name) or opt_attr('name'), etc.",
                "Obtain an attribute value, or an empty string if missing. In \
                Javascript expressions, missing attributes equal to `undefined`.",
            ),
            (
                "attr_del(name), etc.",
                "Obtain an attribute (must be present), simultaneously removing \
                it from the header.",
            ),
            (
                "opt_attr_del(name), etc.",
                "Obtain an attribute (may be missing), simultaneously removing \
                it from the header.",
            ),
            (
                "has_attr(name), etc.",
                "Returns `true` if the given attribute is present, otherwise \
                returns `false`. Especially useful with the `filter` command; \
                equivalent to the expression `opt_attr(name) != undefined`.",
            ),
        ])
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
enum VarType {
    Value,
    Exists,
}

#[derive(Debug)]
struct Var {
    name: String,
    return_type: VarType,
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
    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<bool> {
        let (paction, rtype, allow_missing) = match func.name.as_str() {
            "attr" => (None, VarType::Value, false),
            "has_attr" => (None, VarType::Exists, true),
            "opt_attr" => (None, VarType::Value, true),
            "attr_del" => (Some(attr::Action::Delete), VarType::Value, false),
            "opt_attr_del" => (Some(attr::Action::Delete), VarType::Value, true),
            _ => return Ok(false),
        };
        let name = func.one_arg_as::<String>()?;
        if name.is_empty() {
            return fail!("Attribute names cannot be empty.");
        }
        let attr_id = b.register_attr(&name, paction);
        self.vars.push(Var {
            name: name.to_string(),
            return_type: rtype,
            attr_id,
            id: b.symbol_id(),
            allow_missing,
        });
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set(&mut self, rec: &dyn Record, data: &mut MetaData) -> CliResult<()> {
        for var in &self.vars {
            let sym = data.symbols.get_mut(var.id);
            match var.return_type {
                VarType::Value => {
                    if let Some(val) =
                        data.attrs
                            .get_value(var.attr_id, rec.id_bytes(), rec.desc_bytes())
                    {
                        sym.set_text(val);
                    } else {
                        if !var.allow_missing {
                            return fail!(format!(
                                "Attribute '{}' not found in record '{}'",
                                var.name,
                                String::from_utf8_lossy(rec.id_bytes())
                            ));
                        }
                        sym.set_none();
                    }
                }
                VarType::Exists => {
                    sym.set_bool(data.attrs.has_value(var.attr_id));
                }
            }
        }
        Ok(())
    }
}
