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

impl IoKind {
    pub fn from_path<P: AsRef<Path>>(p: P) -> Result<Self, String> {
        let p = p.as_ref();
        if let Some(s) = p.to_str() {
            Ok(Self::from_str(s).unwrap())
        } else {
            Err(format!("Invalid path: '{}'", p.to_string_lossy()))
        }
    }
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

impl<S> From<S> for IoKind
where
    S: AsRef<str>,
{
    fn from(s: S) -> Self {
        Self::from_str(s.as_ref()).unwrap()
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
