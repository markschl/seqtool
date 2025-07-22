use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::helpers::bytesize::parse_bytesize;
use crate::io::IoKind;

pub const DESC: &str = "\
In the default mode, two files/streams are compared by *ID* (`id` variable) and
*sequence hash* (`seqhash` variable), ignoring descriptions in headers.
The number of common and unique record numbers are reported to STDERR,
unless `-q/--quiet` is specified.

Note that the comparison key can be completely customized with `-k/--key`
(see <KEY> help and `st cmp -V/--help-vars`).

The `-d/--diff` option further allows to compare certain record properties.

If the memory limit is exceeded, two-pass scanning is activated. In this case,
seekable files must be provided.

If the common records in the two input files/streams are known to be in the same
order, `-O/--in-order` can be specified.
This allows fast progressive scanning of the streams, identifying common and
unique records on the fly. The key is allowed to occur multiple times in the input.
Records are assumed to be synchronized and identical as long as the key is identical.
";

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Cmd' command options")]
#[clap(before_help=DESC, help_template=WORDY_HELP)]
pub struct CmpCommand {
    /// The key used to compare the records in the two input files/streams.
    /// Keys must be unique in each input unless `-O/--in-order` is provided.
    /// Can be a single variable/function such as 'id',
    /// a composed string such as '{attr(a)}_{attr(b)}',
    /// or a comma-delimited list of multiple variables/functions,
    /// that are compared separately, e.g. 'id,attr(a)'.
    /// It is also possible to provide `-k/--key` multiple times.
    #[arg(short, long, default_value = "id,seqhash")]
    pub key: Vec<String>,

    /// Comma-delimited list of fields/variabes/expressions, which should be
    /// compared between records that are otherwise identical according to the
    /// comparison key (`-k/--key`).
    /// If two records differ by these extra given properties, a colored alignment
    /// is printed to STDERR.
    #[arg(long, short, value_name = "KEY")]
    pub diff: Option<Vec<String>>,

    /// Provide this option if the two input files/streams are in the same order.
    /// Instead of scanning the whole output, reading and writing is done
    /// progressively. The same key may occur multiple times. Two records are
    /// assumed to be synchronized and idencical as long as they have the same key.
    #[arg(short = 'O', long)]
    pub in_order: bool,

    /// Checks if the two files match exactly, and exits with an error if not.
    #[arg(short, long)]
    pub check: bool,

    #[arg(long = "common", visible_aliases = &["common1", "c1"], value_name = "OUT")]
    /// Write records from the first input to this output file (or `-` for STDOUT)
    /// if *also* present in the second input (according to the comparison of keys).
    pub common_: Option<IoKind>,

    #[arg(long, visible_alias = "c2", value_name = "OUT")]
    /// Write records from the *second* input to the given output (file or `-` for STDOUT)
    /// if *also* present in the first input (according to the comparison of keys).
    pub common2: Option<IoKind>,

    #[arg(long, visible_alias = "u1", value_name = "OUT")]
    /// Write records from the first input to this output file (or `-` for STDOUT)
    /// if *not* present in the second input.
    pub unique1: Option<IoKind>,

    #[arg(long, visible_alias = "u2", value_name = "OUT")]
    /// Write records from the second input to this output file (or `-` for STDOUT)
    /// if *not* present in the first input.
    pub unique2: Option<IoKind>,

    #[arg(long, visible_alias = "o2", value_name = "OUT")]
    /// Write the second input back to this output file (or - for STDOUT).
    /// Useful if only certain aspects of a record are compared, but records
    /// may still differ by other parts.
    pub output2: Option<IoKind>,

    /// Do the comparison in two passes (default: automatically activated if memory limit hit)
    #[arg(short = '2', long, conflicts_with = "in_order")]
    pub two_pass: bool,

    /// Maximum amount of memory (approximate) to use for de-duplicating.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    pub max_mem: usize,

    #[command(flatten)]
    pub common: CommonArgs,
}
