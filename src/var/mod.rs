use var_provider::DynVarProviderInfo;

use self::modules::MODULE_INFO;
use std::io;

pub mod attr;
pub mod build;
pub mod modules;
pub mod parser;
pub mod symbols;
pub mod varstring;

pub use self::build::*;

#[derive(Debug, Clone)]
pub struct VarOpts {
    // metadata
    pub metadata_sources: Vec<String>,
    pub meta_delim_override: Option<u8>,
    pub meta_has_header: bool,
    pub meta_id_col: u32,
    pub meta_dup_ids: bool,
    // expressions
    pub expr_init: Option<String>,
}

#[cold]
pub fn print_var_help(
    custom_help: Option<Box<dyn DynVarProviderInfo>>,
    markdown: bool,
    command_only: bool,
) -> Result<(), io::Error> {
    if let Some(m) = custom_help {
        m.print_help(markdown)?;
    }
    if !command_only {
        for m in MODULE_INFO {
            m.print_help(markdown)?;
        }
    }
    Ok(())
}
