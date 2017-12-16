use io::Record;
use error::CliResult;

use var::*;

pub struct PropHelp;

impl VarHelp for PropHelp {
    fn name(&self) -> &'static str {
        "Properties"
    }
    fn usage(&self) -> &'static str {
        "p:<name>"
    }
    fn desc(&self) -> Option<&'static str> {
        Some("Properties stored in FASTA/FASTQ headers in the form key=value")
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Summarizing over a property in the FASTA header '>id size=3'",
                "seqtool count -k p:size seqs.fa",
            ),
            (
                "Adding the sequence length to the header as property",
                "seqtool . -p seqlen={s:seqlen} seqs.fa",
            ),
        ])
    }
}

#[derive(Debug)]
pub struct PropVars {
    props: Vec<(String, usize, usize)>,
    allow_missing: bool,
}

impl PropVars {
    pub fn new(allow_missing: bool) -> PropVars {
        PropVars {
            props: vec![],
            allow_missing: allow_missing,
        }
    }
}

impl VarProvider for PropVars {
    fn prefix(&self) -> Option<&str> {
        Some("p")
    }

    fn name(&self) -> &'static str {
        "property"
    }

    fn register_var(&mut self, name: &str, id: usize, vars: &mut VarStore) -> CliResult<bool> {
        if name.is_empty() {
            return fail!("Property names cannot be empty.");
        }
        let (paction, name) = if name.as_bytes()[0] == b'-' {
            (Some(prop::Action::Delete), &name[1..])
        } else {
            (None, name)
        };
        let prop_id = vars.register_prop(name, paction);
        self.props.push((name.to_string(), prop_id, id));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.props.is_empty()
    }

    fn set(&mut self, rec: &Record, data: &mut Data) -> CliResult<()> {
        for &(ref name, prop_id, id) in &self.props {
            //let (id_bytes, desc_bytes) = (b"", None); // rec.id_desc_bytes();
            let value: &[u8] = match data.props.get(prop_id, rec.id_bytes(), rec.desc_bytes()) {
                Some(value) => value,
                None => {
                    if !self.allow_missing {
                        return fail!(format!(
                            "Property '{}' not found in record '{}'",
                            name,
                            String::from_utf8_lossy(rec.id_bytes())
                        ));
                    } else {
                        b""
                    }
                }
            };
            data.symbols.set_text(id, value);
        }
        Ok(())
    }
}
