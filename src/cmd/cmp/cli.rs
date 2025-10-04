use std::cell::LazyCell;

use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::helpers::bytesize::parse_bytesize;
use crate::io::IoKind;

pub const DESC: LazyCell<String> = LazyCell::new(|| {
    color_print::cformat!(
        "\
In the default mode, two files/streams are compared by *ID* (`id` variable) and
*sequence hash* (`seqhash` variable), ignoring descriptions in headers.
The number of common and unique record numbers are reported to STDERR,
unless `-q/--quiet` is specified.

Note that the comparison key can be completely customized with `-k/--key`
(see <<KEY>> help and `st cmp -V/--help-vars`).

If the memory limit is exceeded, two-pass scanning is activated. In this case,
seekable files must be provided.

If the the two input files/streams are known to be in sync, then `-O/--in-order`
can be specified for faster comparison and lower memory usage.
The key does not have to be unique in this mode.

<y,s,u>Examples</y,s,u>:

Compare records by ID and sequence (the default mode):

<c>`st cmp file1.fasta file2.fasta`</c>
common	6
unique1	3
unique2	3

Compare only by ID and visualize inconsistencies between sequences:

<c>`st cmp -k id -d seq file1.fasta file2.fasta`</c>
seq_3:
<m>┌</m> CACTTTCAACAACGGATCTCTTG<r>GT</r>TCTCGCATCGATGAAGAACGT<m>┐</m>
<m>└</m> CACTTTCAACAACGGATCTCTTG<c>..</c>TCTCGCATCGATGAAGAACGT<m>┘</m>

common	7
unique1	2
unique2	1
"
    )
});

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Cmp' command options")]
#[clap(before_help=&*DESC, help_template=WORDY_HELP)]
pub struct CmpCommand {
    /// The key used to compare the records in the two input files/streams.
    /// Keys must be unique in each input unless `-O/--in-order` is provided.
    /// Can be a single variable/function such as 'id',
    /// a composed string such as '{attr(a)}_{attr(b)}',
    /// or a comma-delimited list of these.
    /// `-k/--key` may also be provided multiple times, which is equivalent to
    /// a comma-delimited list.
    #[arg(short, long, default_value = "id,seqhash", value_name = "FIELDS")]
    pub key: Vec<String>,

    /// Print differences between the two inputs with respect to one or multiple
    /// extra properties, for records that are otherwise identical according to the
    /// comparison key (`-k/--key`).
    /// If two records differ by these extra given properties, a colored alignment
    /// is printed to STDERR.
    #[arg(long, short, value_name = "FIELDS")]
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

    #[arg(short = '2', long, conflicts_with = "in_order")]
    pub two_pass: bool,

    /// Maximum amount of memory (approximate) to use for de-duplicating.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    pub max_mem: usize,

    /// Maximum width of the `-d/--diff` output
    #[arg(long, value_name = "CHARS", default_value_t = 80)]
    pub diff_width: usize,

    #[command(flatten)]
    pub common: CommonArgs,
}
