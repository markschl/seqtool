use error::CliResult;
use io::Record;

use var::*;

pub struct AttrHelp;

impl VarHelp for AttrHelp {
    fn name(&self) -> &'static str {
        "Attributes"
    }
    fn usage(&self) -> &'static str {
        "a:<name>"
    }
    fn desc(&self) -> Option<&'static str> {
        Some("Attributes stored in FASTA/FASTQ headers in the form key=value")
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Summarizing over an attribute in the FASTA header `>id size=3`",
                "st count -k a:size seqs.fa",
            ),
            (
                "Adding the sequence length to the header as attribute",
                "st . -a seqlen={s:seqlen} seqs.fa",
            ),
        ])
    }
}

#[derive(Debug)]
pub struct AttrVars {
    attrs: Vec<(String, usize, usize)>,
    allow_missing: bool,
}

impl AttrVars {
    pub fn new(allow_missing: bool) -> AttrVars {
        AttrVars {
            attrs: vec![],
            allow_missing: allow_missing,
        }
    }
}

impl VarProvider for AttrVars {
    fn prefix(&self) -> Option<&str> {
        Some("a")
    }

    fn name(&self) -> &'static str {
        "attribute"
    }

    fn register_var(&mut self, name: &str, id: usize, vars: &mut VarStore) -> CliResult<bool> {
        if name.is_empty() {
            return fail!("Attribute names cannot be empty.");
        }
        let (paction, name) = if name.as_bytes()[0] == b'-' {
            (Some(attr::Action::Delete), &name[1..])
        } else {
            (None, name)
        };
        let attr_id = vars.register_attr(name, paction);
        self.attrs.push((name.to_string(), attr_id, id));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.attrs.is_empty()
    }

    fn set(&mut self, rec: &Record, data: &mut Data) -> CliResult<()> {
        for &(ref name, attr_id, id) in &self.attrs {
            //let (id_bytes, desc_bytes) = (b"", None); // rec.id_desc_bytes();
            match data
                .attrs
                .get_value(attr_id, rec.id_bytes(), rec.desc_bytes())
            {
                Some(value) => {
                    data.symbols.set_text(id, value);
                }
                None => {
                    if !self.allow_missing {
                        return fail!(format!(
                            "Attribute '{}' not found in record '{}'",
                            name,
                            String::from_utf8_lossy(rec.id_bytes())
                        ));
                    } else {
                        data.symbols.set_none(id);
                    }
                }
            }
        }
        Ok(())
    }
}
