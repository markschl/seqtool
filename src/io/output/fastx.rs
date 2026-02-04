use std::str::FromStr;

use crate::error::CliResult;
use crate::var::{VarBuilder, attr::AttrWriteAction, varstring::VarString};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

impl FromStr for Attribute {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '=');
        let name = parts.next().unwrap().to_string();
        let value = match parts.next() {
            Some(p) => p.to_string(),
            None => {
                return Err(format!(
                    "Invalid attribute: '{name}'. Attributes need to be in the format: name=value"
                ));
            }
        };
        Ok(Attribute { name, value })
    }
}

pub fn register_attributes(attrs: &[(Attribute, bool)], builder: &mut VarBuilder) -> CliResult<()> {
    for (attr, replace_existing) in attrs {
        let (vs, _) = VarString::parse_register(&attr.value, builder, false)?;
        let action = if *replace_existing {
            AttrWriteAction::Edit(vs)
        } else {
            AttrWriteAction::Append(vs)
        };
        builder.register_attr(&attr.name, Some(action))?;
    }
    Ok(())
}
