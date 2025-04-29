use std::process::exit;

use clap::builder::{
    styling::{AnsiColor, Color, Style},
    Styles,
};
use clap::{value_parser, ArgAction, Args, Parser, Subcommand};

use var_provider::{dyn_var_provider, DynVarProviderInfo};

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::{bytesize::parse_bytesize, seqtype::SeqType};
use crate::io::input::{
    csv::{ColumnMapping, TextColumnSpec},
    InFormat, InputConfig, InputKind, SeqReaderConfig,
};
use crate::io::output::{OutputKind, OutputOpts, SeqWriterOpts};
use crate::io::{FileInfo, FormatVariant, QualFormat};
use crate::var::{attr::AttrFormat, VarOpts};
use crate::{cmd, io::output::fastx::Attribute};

/// This type only serves as a workaround to allow displaying
/// custom help page that explains all variables (--help-vars)
/// The actual CLI interface is defined by `ClapCli`
#[derive(Parser, Debug, Clone)]
struct VarHelpCli {
    command: String,

    #[arg(long)]
    help_vars: bool,

    /// Variable help formatted as Markdown (undocumented)
    // #[cfg(debug_assertions)]
    #[arg(long, hide = true)]
    pub help_vars_md: bool,

    /// Variable help only for current command variable(s), not for common ones (undocumented)
    // #[cfg(debug_assertions)]
    #[arg(long, hide = true)]
    pub help_cmd_vars: bool,
}

#[derive(Debug, Clone)]
pub struct Cli(ClapCli);

impl Cli {
    pub fn new() -> CliResult<Self> {
        // first, try to look for --help-vars using the extra Clap parser
        // in order to work around the fact that clap exits with a
        // 'missing argument error' for command with positional args.
        // TODO: any better way to do this?
        if let Ok(m) = VarHelpCli::try_parse() {
            if m.help_vars || m.help_vars_md {
                let custom_help: Option<Box<dyn DynVarProviderInfo>> = match m.command.as_str() {
                    #[cfg(any(feature = "all-commands", feature = "sort"))]
                    "sort" => Some(Box::new(dyn_var_provider!(cmd::sort::SortVar))),
                    #[cfg(any(feature = "all-commands", feature = "unique"))]
                    "unique" => Some(Box::new(dyn_var_provider!(cmd::unique::UniqueVar))),
                    #[cfg(any(feature = "all-commands", feature = "split"))]
                    "split" => Some(Box::new(dyn_var_provider!(cmd::split::SplitVar))),
                    #[cfg(any(feature = "all-commands", feature = "find"))]
                    "find" => Some(Box::new(dyn_var_provider!(cmd::find::FindVar))),
                    _ => None,
                };
                crate::var::print_var_help(custom_help, m.help_vars_md, m.help_cmd_vars)?;
                exit(0);
            }
        }
        Ok(Self(ClapCli::parse()))
    }

    pub fn run(&mut self) -> CliResult<()> {
        use SubCommand::*;
        macro_rules! run {
            ($cmdmod:ident, $opts:expr) => {
                cmd::$cmdmod::run(Config::new(&$opts.common)?, $opts)
            };
        }
        match self.0.command {
            #[cfg(any(feature = "all-commands", feature = "pass"))]
            Pass(ref opts) => run!(pass, opts),
            #[cfg(any(feature = "all-commands", feature = "view"))]
            View(ref opts) => run!(view, opts),
            #[cfg(any(feature = "all-commands", feature = "count"))]
            Count(ref opts) => run!(count, opts),
            #[cfg(any(feature = "all-commands", feature = "stat"))]
            Stat(ref opts) => run!(stat, opts),
            #[cfg(any(feature = "all-commands", feature = "head"))]
            Head(ref opts) => run!(head, opts),
            #[cfg(any(feature = "all-commands", feature = "tail"))]
            Tail(ref opts) => run!(tail, opts),
            #[cfg(any(feature = "all-commands", feature = "slice"))]
            Slice(ref opts) => run!(slice, opts),
            #[cfg(any(feature = "all-commands", feature = "sample"))]
            Sample(ref opts) => run!(sample, opts),
            #[cfg(any(feature = "all-commands", feature = "sort"))]
            Sort(ref opts) => run!(sort, opts),
            #[cfg(any(feature = "all-commands", feature = "unique"))]
            Unique(ref opts) => run!(unique, opts),
            #[cfg(any(
                all(feature = "expr", feature = "all-commands"),
                all(feature = "expr", feature = "filter")
            ))]
            Filter(ref opts) => run!(filter, opts),
            #[cfg(any(feature = "all-commands", feature = "split"))]
            Split(ref opts) => run!(split, opts),
            #[cfg(any(feature = "all-commands", feature = "interleave"))]
            Interleave(ref opts) => run!(interleave, opts),
            #[cfg(any(feature = "all-commands", feature = "find"))]
            Find(ref opts) => run!(find, opts),
            #[cfg(any(feature = "all-commands", feature = "replace"))]
            Replace(ref opts) => run!(replace, opts),
            #[cfg(any(feature = "all-commands", feature = "set"))]
            Set(ref opts) => run!(set, opts),
            #[cfg(any(feature = "all-commands", feature = "del"))]
            Del(ref opts) => run!(del, opts),
            #[cfg(any(feature = "all-commands", feature = "trim"))]
            Trim(ref opts) => run!(trim, opts),
            #[cfg(any(feature = "all-commands", feature = "mask"))]
            Mask(ref opts) => run!(mask, opts),
            #[cfg(any(feature = "all-commands", feature = "upper"))]
            Upper(ref opts) => run!(upper, opts),
            #[cfg(any(feature = "all-commands", feature = "lower"))]
            Lower(ref opts) => run!(lower, opts),
            #[cfg(any(feature = "all-commands", feature = "revcomp"))]
            Revcomp(ref opts) => run!(revcomp, opts),
            #[cfg(any(feature = "all-commands", feature = "concat"))]
            Concat(ref opts) => run!(concat, opts),
        }
    }
}

impl CommonArgs {
    pub fn get_input_cfg(&self) -> CliResult<Vec<(InputConfig, SeqReaderConfig)>> {
        let args = &self.input;

        // get format settings from args
        let mut info = args.fmt.clone();
        let mut delim = args.delim;
        let mut fields = args.fields.clone();

        // --fa/--fq/--tsv, etc have a higher priority
        if args.fa {
            info = Some(FileInfo::new(FormatVariant::Fasta, None));
        } else if args.fq {
            info = Some(FileInfo::new(
                FormatVariant::Fastq(QualFormat::Sanger),
                None,
            ));
        } else if args.fq_illumina {
            info = Some(FileInfo::new(
                FormatVariant::Fastq(QualFormat::Illumina),
                None,
            ));
        } else if args.csv.is_some() {
            info = Some(FileInfo::new(FormatVariant::Csv, None));
            delim = Some(',');
            fields.clone_from(&args.csv);
        } else if args.tsv.is_some() {
            info = Some(FileInfo::new(FormatVariant::Tsv, None));
            delim = Some('\t');
            fields.clone_from(&args.tsv);
        }

        let input: Vec<_> = args
            .input
            .iter()
            .map(|kind| {
                // if no format from args, infer from path
                let mut _info = info.clone().unwrap_or_else(|| kind.get_info());

                let format = InFormat::from_opts(
                    _info.format,
                    delim,
                    fields.as_deref(),
                    args.header,
                    args.qual.as_deref(),
                )?;

                let input_cfg = InputConfig {
                    kind: kind.clone(),
                    compression: _info.compression,
                    threaded: self.advanced.read_thread,
                    thread_bufsize: self.advanced.read_tbufsize,
                };
                let seq_cfg = SeqReaderConfig {
                    format,
                    seqtype: args.seqtype,
                    cap: self.advanced.buf_cap,
                    max_mem: self.advanced.max_read_mem,
                };
                Ok((input_cfg, seq_cfg))
            })
            .collect::<CliResult<_>>()?;
        Ok(input)
    }

    pub fn get_output_opts(&self) -> CliResult<(OutputKind, OutputOpts, SeqWriterOpts)> {
        let args = &self.output;

        // format
        let mut info = args.to.clone();
        let mut delim = args.out_delim;
        let mut fields = args.outfields.clone();

        // furthermore, --fa/--fq/--tsv, etc. override --to <format>
        // (no compression possible)
        if args.to_fa {
            info = Some(FileInfo::new(FormatVariant::Fasta, None));
        } else if args.to_fq {
            info = Some(FileInfo::new(
                FormatVariant::Fastq(QualFormat::Sanger),
                None,
            ));
        } else if let Some(f) = args.to_csv.as_ref() {
            info = Some(FileInfo::new(FormatVariant::Csv, None));
            delim = Some(',');
            fields = Some(f.clone());
        } else if let Some(f) = args.to_tsv.as_ref() {
            info = Some(FileInfo::new(FormatVariant::Tsv, None));
            delim = Some('\t');
            fields = Some(f.clone());
        }

        // assemble attributes
        let mut attrs = Vec::with_capacity(self.attr.attr.len() + self.attr.attr_append.len());
        for a in &self.attr.attr {
            attrs.push((a.clone(), true));
        }
        for a in &self.attr.attr_append {
            attrs.push((a.clone(), false));
        }

        let output_opts = OutputOpts {
            append: args.append,
            compression_format: info.as_ref().and_then(|f| f.compression),
            compression_level: args.compr_level,
            threaded: self.advanced.write_thread,
            thread_bufsize: self.advanced.write_tbufsize,
        };

        let format_opts = SeqWriterOpts {
            format: info.map(|f| f.format),
            attrs,
            wrap_fasta: args.wrap.map(|w| w as usize),
            fields,
            delim,
            qfile: args.qual_out.clone(),
        };

        let kind = args.output.clone().unwrap_or(OutputKind::Stdout);

        Ok((kind, output_opts, format_opts))
    }

    pub fn get_var_opts(&self) -> CliResult<VarOpts> {
        Ok(VarOpts {
            metadata_sources: self.meta.meta.clone(),
            meta_delim_override: self.meta.meta_delim.map(|d| d as u8),
            meta_has_header: self.meta.meta_header,
            meta_id_col: self.meta.meta_idcol.checked_sub(1).unwrap(),
            meta_dup_ids: self.meta.dup_ids,
            expr_init: self.expr.js_init.clone(),
        })
    }
}

/// help template for individual subcommands, where
/// most importantly, "about" comes *before* "before_help",
/// so we can force a longer multi-line description at the
/// top even in the short help page
pub const WORDY_HELP: &str = "\
{about-with-newline}
{before-help}
{usage-heading} {usage}

{all-args}{after-help}";

#[derive(Parser, Clone, Debug)]
#[command(version, about)]
#[command(styles=get_styles())]
pub struct ClapCli {
    #[command(subcommand)]
    pub command: SubCommand,
}

/// Commands (optional)
#[derive(Subcommand, Clone, Debug)]
pub enum SubCommand {
    /// Directly pass input to output without any processing,
    /// useful for converting and attribute setting
    #[cfg(any(feature = "all-commands", feature = "pass"))]
    #[command(visible_aliases=&["."])]
    Pass(cmd::pass::PassCommand),
    /// View biological sequences, colored by base / amino acid, or by sequence quality.
    #[cfg(any(feature = "all-commands", feature = "view"))]
    View(cmd::view::ViewCommand),
    /// Count all records in the input (total or categorized by variables/functions)
    #[cfg(any(feature = "all-commands", feature = "count"))]
    Count(cmd::count::CountCommand),
    /// Return per-sequence statistics as tab delimited list
    #[cfg(any(feature = "all-commands", feature = "stat"))]
    Stat(cmd::stat::StatCommand),
    /// Return the first N sequences
    #[cfg(any(feature = "all-commands", feature = "head"))]
    Head(cmd::head::HeadCommand),
    /// Return the last N sequences
    #[cfg(any(feature = "all-commands", feature = "tail"))]
    Tail(cmd::tail::TailCommand),
    /// Return a range of sequence records from the input
    #[cfg(any(feature = "all-commands", feature = "slice"))]
    Slice(cmd::slice::SliceCommand),
    /// Get a random subset of sequences; either a fixed number
    /// or an approximate fraction of the input.
    #[cfg(any(feature = "all-commands", feature = "sample"))]
    Sample(cmd::sample::SampleCommand),
    /// Sort records by sequence or any other criterion
    #[cfg(any(feature = "all-commands", feature = "sort"))]
    Sort(cmd::sort::cli::SortCommand),
    /// De-replicate by sequence and/or other properties, returning only
    /// unique records
    #[cfg(any(feature = "all-commands", feature = "unique"))]
    Unique(cmd::unique::cli::UniqueCommand),
    /// Keep/exclude sequences based on different properties with a
    /// mathematical (JavaScript) expression
    #[cfg(any(
        all(feature = "expr", feature = "all-commands"),
        all(feature = "expr", feature = "filter")
    ))]
    Filter(cmd::filter::FilterCommand),
    /// Distribute sequences into multiple files based on a variable/function
    /// or advanced expression.
    #[cfg(any(feature = "all-commands", feature = "split"))]
    Split(cmd::split::SplitCommand),
    /// Interleave records of all files in the input.
    #[cfg(any(feature = "all-commands", feature = "interleave"))]
    Interleave(cmd::interleave::InterleaveCommand),
    /// Search for pattern(s) in sequences or sequene headers
    /// for record filtering, pattern replacement or passing hits to next command.
    #[cfg(any(feature = "all-commands", feature = "find"))]
    Find(cmd::find::FindCommand),
    /// Fast and simple pattern replacement in sequences or headers
    #[cfg(any(feature = "all-commands", feature = "replace"))]
    Replace(cmd::replace::ReplaceCommand),
    /// Replace the header, header attributes or sequence with new content
    #[cfg(any(feature = "all-commands", feature = "set"))]
    Set(cmd::set::SetCommand),
    /// Delete header ID/description and/or attributes
    #[cfg(any(feature = "all-commands", feature = "del"))]
    Del(cmd::del::DelCommand),
    /// Trim sequences on the left and/or right (single range)
    /// or extract and concatenate several ranges.
    #[cfg(any(feature = "all-commands", feature = "trim"))]
    Trim(cmd::trim::TrimCommand),
    /// Soft or hard mask sequence ranges
    #[cfg(any(feature = "all-commands", feature = "mask"))]
    Mask(cmd::mask::MaskCommand),
    /// Convert sequences to uppercase
    #[cfg(any(feature = "all-commands", feature = "upper"))]
    Upper(cmd::upper::UpperCommand),
    /// Convert sequences to lowercase
    #[cfg(any(feature = "all-commands", feature = "lower"))]
    Lower(cmd::lower::LowerCommand),
    /// Reverse complements DNA or RNA sequences
    #[cfg(any(feature = "all-commands", feature = "revcomp"))]
    Revcomp(cmd::revcomp::RevcompCommand),
    /// Concatenates sequences/alignments from different files
    #[cfg(any(feature = "all-commands", feature = "concat"))]
    Concat(cmd::concat::ConcatCommand),
}

/// Common options
#[derive(Args, Clone, Debug)]
pub struct CommonArgs {
    #[command(flatten)]
    pub general: GeneralArgs,

    #[command(flatten)]
    pub input: InputArgs,

    #[command(flatten)]
    pub output: OutputArgs,

    #[command(flatten)]
    pub attr: AttrArgs,

    #[command(flatten)]
    pub meta: MetaArgs,

    #[command(flatten)]
    pub expr: ExprArgs,

    #[command(flatten)]
    pub advanced: AdvancedArgs,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "General options (common to all commands)")]
pub struct GeneralArgs {
    /// Print more detailed information about the progress and results of certain commands
    #[arg(short, long)]
    pub verbose: bool,

    /// Suppress all messages except errors and important warnings
    #[arg(short, long)]
    pub quiet: bool,

    /// List and explain all variables/functions available
    #[arg(long)]
    pub help_vars: bool,
}

/// Input options
#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Input (common to all commands)")]
pub struct InputArgs {
    /// Input file(s), multiple possible (use '-' for STDIN)
    #[arg(default_value = "-")]
    pub input: Vec<InputKind>,

    #[arg(long, env = "ST_FORMAT")]
    /// Input format, only needed if it cannot be guessed from the extension
    /// (e.g. if reading from STDIN). 'fasta' is assumed as default
    /// (can be configured with ST_FORMAT). Possible choices:
    /// fasta (default), fastq (fastq-illumina, fastq-solexa),
    /// csv or tsv.
    /// Compression: <format>.<compression> (.gz, .bz2 or .lz4).
    pub fmt: Option<FileInfo>,

    #[arg(long)]
    /// FASTA input. Short for '--fmt fasta'.
    pub fa: bool,

    #[arg(long)]
    /// FASTQ input. Short for '--fmt fastq'.
    pub fq: bool,

    #[arg(long)]
    /// FASTQ input in legacy Illumina 1.3-1.7 format (alias to --fmt fastq-illumina)
    pub fq_illumina: bool,

    #[arg(
        long,
        value_name = "FIELDS",
        value_parser = parse_infields,
    )]
    /// Delimited text fields:
    /// 'id,seq,desc' (in order) or
    /// 'id:2,desc:6,seq:9' (col. num.) or
    /// 'id:ID,seq:Sequence,desc:Comment' (names in header)
    /// [default: 'id,seq,desc']
    pub fields: Option<Vec<ColumnMapping>>,

    #[arg(long, value_name = "CHAR")]
    /// TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for csv
    pub delim: Option<char>,

    #[arg(long)]
    /// Specify if CSV file has a header. Auto-enabled if a 'field:column name'
    /// mapping is provided with --fields, --csv or --tsv
    pub header: bool,

    #[arg(long, value_name = "FIELDS", value_parser = parse_infields)]
    /// CSV input. Short for '--fmt csv --fields <fields>'
    pub csv: Option<Vec<ColumnMapping>>,

    #[arg(long, value_name = "FIELDS", value_parser = parse_infields)]
    /// TSV input. Short for '--fmt tsv --fields <fields>'
    pub tsv: Option<Vec<ColumnMapping>>,

    #[arg(long, value_name = "FILE")]
    /// Path to QUAL file with quality scores (Roche 454 style)
    pub qual: Option<String>,

    /// Sequence type; relevant for the `find` and `revcomp` commands,
    /// as well as the variables/functions `seq_revcomp`, `seqhash_rev` and `seqhash_both`
    /// (default: auto-detected based on the first sequence)
    #[arg(long, value_enum, value_name = "TYPE")]
    pub seqtype: Option<SeqType>,
}

/// Your application's description
#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Output (common to all commands)")]
pub struct OutputArgs {
    #[arg(short, long, value_name = "FILE")]
    /// Write output to <file> instead of STDOUT [Default: STDOUT (-)]
    pub output: Option<OutputKind>,

    /// Append sequences to the end if the output file(s) already exist instead of
    /// replacing the content. In case writing to standard output
    /// (which is the default if `-o/--output` is not specified), this option
    /// has no effect.
    #[arg(long)]
    append: bool,

    #[arg(long, value_name = "FORMAT")]
    /// Output format and compression. See --fmt.
    /// Only needed if not guessed from the extension (default: input format).
    pub to: Option<FileInfo>,

    #[arg(long, value_name = "WIDTH", value_parser = value_parser!(u32).range(1..))]
    /// Wrap FASTA sequences to maximum <width> characters
    pub wrap: Option<u32>,

    #[arg(long, value_name = "DELIM")]
    /// TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for csv
    pub out_delim: Option<char>,

    #[arg(long, value_name = "FIELDS")]
    /// Comma delimited list of CSV/TSV fields, which can be
    /// variables/functions or contain {variables}/{expressions}.
    /// [default: input fields or 'id,desc,seq']
    pub outfields: Option<String>,

    #[arg(long)]
    /// FASTA output. Short for: '--to fasta'
    pub to_fa: bool,

    #[arg(long)]
    /// FASTQ output. Short for: '--to fastq'
    pub to_fq: bool,

    /// CSV output with comma delimited list of fields, which can be
    /// variables/functions or contain variables/expressions.
    /// Short for '--to csv --outfields <f>'
    #[arg(long, value_name = "FIELDS")]
    pub to_csv: Option<String>,

    /// TSV output with comma delimited list of fields, which can be
    /// variables/functions or contain variables/expressions.
    /// Short for '--to tsv --outfields <f>'
    #[arg(long, value_name = "FIELDS")]
    pub to_tsv: Option<String>,

    /// Level for compressed output. 1-9 for GZIP/BZIP2 (default=6) and
    /// 1-16 for LZ4 (default=0). 1-22 for Zstandard (default=3 or 0)
    #[arg(long, value_name = "L", value_parser = value_parser!(u8).range(0..=22))]
    pub compr_level: Option<u8>,

    /// Path to QUAL output file with quality scores
    #[arg(long, value_name = "FILE")]
    pub qual_out: Option<String>,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "FASTA/Q header attributes (all commands)")]
pub struct AttrArgs {
    /// Add an attribute in the form name=value to FASTA/FASTQ
    /// headers or replace their value if the given name already exists
    /// (multiple -a key=value arguments possible).
    /// The default output format is: '>id some description key1=value1 key2=value2'.
    /// Use --attr-format to change.
    #[arg(short, long, value_name = "KEY=VALUE", action = ArgAction::Append)]
    pub attr: Vec<Attribute>,

    /// Append one or multiple attributes in the form name=value to FASTA/FASTQ
    /// headers. Compared to `-a/--attr`, existing attributes in headers are NOT
    /// replaced. This will result in a duplicate entry if the given attribute
    /// name already exists.
    #[arg(short = 'A', long, value_name = "K=V", action = ArgAction::Append)]
    pub attr_append: Vec<Attribute>,

    /// Expected format of sequence header attributes, which is also used
    /// for writing new attributes to headers (using -a/--attr).
    /// The words 'key' and 'value' must always be present, and 'value'
    /// must follow after 'key'. Example: ';key=value'. If the delimiter
    /// before the key is not a space attributes are appended to the ID
    /// (part before the first space) instead of the end of the header.
    #[arg(
        long,
        env = "ST_ATTR_FORMAT",
        value_name = "FMT",
        default_value = " key=value"
    )]
    pub attr_fmt: AttrFormat,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Associated metadata (all commands)")]
pub struct MetaArgs {
    /// Delimited text file path (or '-' for STDIN) containing associated metadata,
    /// accessed using the `meta(field)` function, or `meta(field, file-num)` in case
    /// of multiple metadata files (supplied like this: -m file1 -m file2 ...).
    #[arg(short, long, value_name = "FILE", action = ArgAction::Append)]
    pub meta: Vec<String>,

    /// Metadata column delimiter. Inferred from the file extension if possible:
    /// '.csv' is interpreted as comma(,)-delimited, '.tsv'/'.txt' or other (unknown)
    /// extensions are assumed to be tab-delimited.
    #[arg(long, value_name = "CHAR", default_value = "\t")]
    pub meta_delim: Option<char>,

    /// Specify if the first row of the metadata file(s) contains column names.
    /// Automatically enabled if a non-numeric field names are used, e.g. 'meta(fieldname)'.
    #[arg(long)]
    pub meta_header: bool,

    /// Column number containing the sequence record IDs
    #[arg(long, value_name = "NUM", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    pub meta_idcol: u32,

    /// Specify if the sequence input is expected to contain duplicate IDs.
    /// Without this flag, there may be an error (`meta` and `has_meta` functions),
    /// whereas `opt_meta` may wrongly return missing values.
    #[arg(long)]
    pub dup_ids: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Expressions/scripts (all commands)")]
pub struct ExprArgs {
    /// Javascript code to execute during initialization
    /// (e.g. for defining global variables used later during parsing).
    /// Either a plain string or 'file:path/to/file.js'
    #[arg(long, value_name = "CODE")]
    pub js_init: Option<String>,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Advanced (all commands)")]
pub struct AdvancedArgs {
    /// Initial capacity of internal FASTA/FASTQ reader buffer. Either a plain number (bytes)
    /// a number with unit (K, M, G, T) based on powers of 2.
    #[arg(long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "64K", hide = true)]
    pub buf_cap: usize,

    /// Buffer size limit for the internal FASTA/FASTQ reader. Larger sequence records will
    /// cause an error. Note, that some commands such as 'sort', 'unique'
    /// and 'sample' still use more memory and have their own additional
    /// memory limit setting.
    /// Either a plain number (bytes) a number with unit (K, M, G, T)
    /// based on powers of 2.
    #[arg(long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "1G")]
    pub max_read_mem: usize,

    /// Read from a different thread. Enabled with compressed input.
    #[arg(short('T'), long)]
    pub read_thread: bool,

    /// Write in a different thread. Enabled with compressed output.
    #[arg(short('W'), long)]
    pub write_thread: bool,

    /// Buffer size of background reader.
    /// Plain number (bytes), a number with unit (K, M, G, T).
    /// The default is 4 MiB or the optimal size depending on the compression format.
    #[arg(long, value_name = "N", value_parser = parse_bytesize, hide = true)]
    pub read_tbufsize: Option<usize>,

    /// Buffer size of background writer.
    /// Plain number (bytes), a number with unit (K, M, G, T).
    /// The default is 4 MiB or the optimal size depending on the compression format.
    #[arg(long, value_name = "N", value_parser = parse_bytesize, hide = true)]
    pub write_tbufsize: Option<usize>,
}

pub fn parse_infields(field_str: &str) -> CliResult<Vec<ColumnMapping>> {
    let fields: Vec<_> = field_str
        .split(',')
        .map(|field| {
            let mut it = field.splitn(2, ':');
            (it.next().unwrap().trim(), it.next().map(|f| f.trim()))
        })
        .collect();

    let has_colmapping = fields[0].1.is_some();
    if fields.iter().any(|(_, f)| has_colmapping != f.is_some()) {
        return fail!(
            "Inconsistent text column description: '{}'. \
            Either use 'field1,field2,...' (in-order) or 'field1:column1,field2:column2,...', \
            but do not mix the two.",
            field_str
        );
    }

    if has_colmapping {
        let maybe_pos: Result<Vec<_>, _> = fields
            .iter()
            .map(|(_, f)| f.unwrap().parse::<usize>())
            .collect();
        if let Ok(pos) = maybe_pos {
            pos.into_iter()
                .zip(&fields)
                .map(|(pos, (attr, _))| {
                    if pos == 0 {
                        fail!(
                            "Invalid column number for '{}': numbers should be > 0",
                            attr
                        )
                    } else {
                        Ok((attr.to_string(), TextColumnSpec::Index(pos - 1)))
                    }
                })
                .collect()
        } else {
            Ok(fields
                .into_iter()
                .map(|(attr, field)| {
                    (
                        attr.to_string(),
                        TextColumnSpec::Name(field.unwrap().to_string()),
                    )
                })
                .collect())
        }
    } else {
        Ok(fields
            .into_iter()
            .enumerate()
            .map(|(i, (attr, _))| (attr.to_string(), TextColumnSpec::Index(i)))
            .collect())
    }
}

pub fn get_styles() -> Styles {
    Styles::styled()
        .usage(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .header(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::White))))
}
