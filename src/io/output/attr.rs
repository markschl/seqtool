use crate::error::CliResult;
use crate::io::Attribute;
use crate::var::{attr::AttrWriteAction, varstring::VarString, VarBuilder};

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
