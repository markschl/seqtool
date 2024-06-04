use std::path::PathBuf;

use clap::Parser;

use crate::cli::{CommonArgs, WORDY_HELP};
use crate::helpers::bytesize::parse_bytesize;

pub const DESC: &str = "\
The sort key can be 'seq', 'id', or any variable/function, expression, or
text containing them (see <KEY> help and `st sort --help-vars`).

Records with identical keys are kept in input order.

The actual value of the key is available through the 'key' variable. It can
be written to a header attribute or TSV field.
";

lazy_static::lazy_static! {
    pub static ref EXAMPLES: String = color_print::cformat!(
        "\
 <c>st sort seqlen input.fasta</c><r>
>>id10
SEQ
>>id3
SEQUE
>>id1
SEQUENCE
</r>

Write the sort key (obtained from JS expression, whose evaluation takes time)
to a header attribute:

<c>st sort -n '{{ id.substring(2, 5) }}' -a id_num='{{num(key)}}' input.fasta</c><r>
>>id001 id_num=1
SEQ
>>id002 id_num=2
SEQ
</r>\
"
);
}

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "'Sort' command options")]
#[clap(before_help=DESC, after_help=&*EXAMPLES, help_template=WORDY_HELP)]
pub struct SortCommand {
    /// The key used to sort the records. It can be a single variable/function
    /// such as 'seq', 'id', a composed string, e.g. '{id}_{desc}',
    /// or a comma-delimited list of multiple variables/functions to sort by,
    /// e.g. 'seq,attr(a)'. In this case, the records are first sorted by sequence,
    /// but in case of identical sequences, records are sorted by the header
    /// attribute 'a'.
    ///
    /// To sort by a numeric FASTA attribute in the form '>id;size=123':
    /// `st sort 'num(attr(size))' --attr-fmt ';key=value' input.fasta`.
    ///
    /// Regarding formulas returning mixed text/numbers, the sorted records with
    /// text keys will be returned first and the sorted number records after them.
    /// Furthermore, NaN and missing values (null/undefined in JS expressions,
    /// missing `opt_attr()` values or missing entries in associated metadata)
    /// will appear last.
    pub key: String,

    /// Sort in reverse order
    #[arg(short, long)]
    pub reverse: bool,

    /// Maximum amount of memory (approximate) to use for sorting.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(short = 'M', long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "5G")]
    pub max_mem: usize,

    /// Path to temporary directory (only if memory limit is exceeded)
    #[arg(long, value_name = "PATH")]
    pub temp_dir: Option<PathBuf>,

    /// Maximum number of temporary files allowed
    #[arg(long, value_name = "N", default_value_t = 1000)]
    pub temp_file_limit: usize,

    #[command(flatten)]
    pub common: CommonArgs,
}
