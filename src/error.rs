use std::borrow::ToOwned;
use std::convert::From;
use std::fmt;
use std::io;
use std::error;
use std::str::Utf8Error;
use std::num::{ParseFloatError, ParseIntError};
use std::string::FromUtf8Error;

use seq_io;
use docopt;
use regex;
#[cfg(feature = "exprtk")]
use exprtk_rs;

//use lib::selective_csv;
use csv;
use meval;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    Io(io::Error),
    Other(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CliError::Io(ref e) => e.fmt(f),
            CliError::Other(ref s) => f.write_str(&**s),
        }
    }
}

impl error::Error for CliError {
    fn description(&self) -> &str {
        "seqtool commandline error"
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> CliError {
        CliError::Io(err)
    }
}

impl<'a> From<&'a str> for CliError {
    fn from(err: &'a str) -> CliError {
        CliError::Other(err.to_owned())
    }
}

impl<W> From<csv::IntoInnerError<W>> for CliError {
    fn from(err: csv::IntoInnerError<W>) -> CliError {
        CliError::Other(format!("{}", err))
    }
}

macro_rules! from_err(($e:ty) => (
    impl From<$e> for CliError {
        fn from(err: $e) -> CliError {
            CliError::Other(format!("{}", err))
        }
    }
));

from_err!(String);
from_err!(fmt::Error);
from_err!(seq_io::fasta::Error);
from_err!(seq_io::fastq::Error);
from_err!(docopt::Error);
from_err!(regex::Error);
from_err!(Utf8Error);
from_err!(FromUtf8Error);
from_err!(ParseIntError);
from_err!(ParseFloatError);
from_err!(csv::Error);
from_err!(meval::Error);
#[cfg(feature = "exprtk")]
from_err!(exprtk_rs::ParseError);
#[cfg(feature = "exprtk")]
from_err!(exprtk_rs::InvalidName);
