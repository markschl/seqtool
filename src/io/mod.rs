use std::convert::Infallible;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub use self::format::*;
pub use self::qual_format::*;
pub use self::record::*;

mod format;
pub mod input;
pub mod output;
mod qual_format;
mod record;

pub const DEFAULT_FORMAT: FormatVariant = FormatVariant::Fasta;

pub const DEFAULT_IO_READER_BUFSIZE: usize = 1 << 22;
pub const DEFAULT_IO_WRITER_BUFSIZE: usize = 1 << 22;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum IoKind {
    Stdio,
    File(PathBuf),
}

impl FromStr for IoKind {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-" {
            Ok(Self::Stdio)
        } else {
            Ok(Self::File(s.into()))
        }
    }
}

impl TryFrom<&Path> for IoKind {
    type Error = String;

    fn try_from(p: &Path) -> Result<Self, Self::Error> {
        if let Some(s) = p.to_str() {
            Ok(Self::from_str(s).unwrap())
        } else {
            Err(format!("Invalid path: '{}'", p.to_string_lossy()))
        }
    }
}

impl fmt::Display for IoKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Stdio => write!(f, "-"),
            Self::File(ref p) => write!(f, "{}", p.as_path().to_string_lossy()),
        }
    }
}
