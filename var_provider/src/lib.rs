use strum_macros::{Display, EnumString};

mod func;
mod usage;
mod var_provider;

pub use self::func::*;
pub use self::usage::*;
pub use self::var_provider::*;

/// Provides information about the expected variable/function output type
#[derive(Debug, Clone, EnumString, Display)]
#[strum(serialize_all = "snake_case")]
pub enum VarType {
    Text,
    Number,
    Boolean,
}
