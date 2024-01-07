use crate::cmd;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::bytesize::parse_bytesize;
use crate::io::Compression;
use crate::io::{
    input::*,
    output::{OutFormat, OutputKind, OutputOptions},
    Attribute, FileInfo, FormatVariant, QualFormat,
};
use crate::var::{AttrOpts, VarOpts};

use clap::value_parser;
use clap::{ArgAction, Args, Parser, Subcommand};

#[derive(Debug, Clone)]
pub struct Cli(ClapCli);

impl Cli {
    pub fn new() -> Self {
        Self(ClapCli::parse())
    }

    pub fn run(&mut self) -> CliResult<()> {
        use SubCommand::*;
        macro_rules! run {
            ($cmdmod:ident, $opts:expr) => {
                cmd::$cmdmod::run(Config::new(&$opts.common)?, $opts)
            };

            ($cmdmod:ident, $opts:expr, $var_mod:expr) => {
                cmd::$cmdmod::run(Config::with_vars(&$opts.common, $var_mod)?, $opts)
            };
        }
        match self.0.command {
            #[cfg(any(feature = "all_commands", feature = "pass"))]
            Pass(ref opts) => run!(pass, opts),
            #[cfg(any(feature = "all_commands", feature = "view"))]
            View(ref opts) => run!(view, opts),
            #[cfg(any(feature = "all_commands", feature = "count"))]
            Count(ref opts) => run!(count, opts),
            #[cfg(any(feature = "all_commands", feature = "stat"))]
            Stat(ref opts) => run!(stat, opts),
            #[cfg(any(feature = "all_commands", feature = "head"))]
            Head(ref opts) => run!(head, opts),
            #[cfg(any(feature = "all_commands", feature = "tail"))]
            Tail(ref opts) => run!(tail, opts),
            #[cfg(any(feature = "all_commands", feature = "slice"))]
            Slice(ref opts) => run!(slice, opts),
            #[cfg(any(feature = "all_commands", feature = "sample"))]
            Sample(ref opts) => run!(sample, opts),
            #[cfg(any(feature = "all_commands", feature = "sort"))]
            Sort(ref opts) => run!(
                sort,
                opts,
                Some(Box::<cmd::shared::key_var::KeyVars>::default())
            ),
            #[cfg(any(feature = "all_commands", feature = "unique"))]
            Unique(ref opts) => run!(
                unique,
                opts,
                Some(Box::<cmd::shared::key_var::KeyVars>::default())
            ),
            #[cfg(any(feature = "all_commands", all(feature = "expr", feature = "filter")))]
            Filter(ref opts) => run!(filter, opts),
            #[cfg(any(feature = "all_commands", feature = "split"))]
            Split(ref opts) => run!(split, opts, cmd::split::get_split_vars(opts)),
            #[cfg(any(feature = "all_commands", feature = "interleave"))]
            Interleave(ref opts) => run!(interleave, opts),
            #[cfg(any(feature = "all_commands", feature = "find"))]
            Find(ref opts) => run!(
                find,
                opts,
                Some(Box::new(cmd::find::FindVars::new(opts.patterns.len())))
            ),
            #[cfg(any(feature = "all_commands", feature = "replace"))]
            Replace(ref opts) => run!(replace, opts),
            #[cfg(any(feature = "all_commands", feature = "set"))]
            Set(ref opts) => run!(set, opts),
            #[cfg(any(feature = "all_commands", feature = "del"))]
            Del(ref opts) => run!(del, opts),
            #[cfg(any(feature = "all_commands", feature = "trim"))]
            Trim(ref opts) => run!(trim, opts),
            #[cfg(any(feature = "all_commands", feature = "mask"))]
            Mask(ref opts) => run!(mask, opts),
            #[cfg(any(feature = "all_commands", feature = "upper"))]
            Upper(ref opts) => run!(upper, opts),
            #[cfg(any(feature = "all_commands", feature = "lower"))]
            Lower(ref opts) => run!(lower, opts),
            #[cfg(any(feature = "all_commands", feature = "revcomp"))]
            Revcomp(ref opts) => run!(revcomp, opts),
            #[cfg(any(feature = "all_commands", feature = "concat"))]
            Concat(ref opts) => run!(concat, opts),
        }
    }
}

impl CommonArgs {
    pub fn get_input_opts(&self) -> CliResult<Vec<InputOptions>> {
        let opts = &self.input;
        // TODO: ST_FORMAT removed

        // get format settings from args
        let mut delim = opts.delim;
        let mut fields = opts.fields.clone();
        let info = opts.fmt.clone();

        let input: Vec<_> = opts
            .input
            .iter()
            .map(|kind| {
                // if no format from args, infer from path
                let mut _info = info.clone().unwrap_or_else(|| match kind {
                    InputKind::Stdin => FileInfo::new(FormatVariant::Fasta, Compression::None),
                    InputKind::File(path) => FileInfo::from_path(path, FormatVariant::Fasta),
                });

                // --fa/--fq/--tsv, etc have highest priority
                let compr = _info.compression;
                if opts.fa {
                    _info = FileInfo::new(FormatVariant::Fasta, compr);
                } else if opts.fq {
                    _info = FileInfo::new(FormatVariant::Fastq(QualFormat::Sanger), compr);
                } else if opts.fq_illumina {
                    _info = FileInfo::new(FormatVariant::Fastq(QualFormat::Illumina), compr);
                } else if let Some(f) = opts.csv.as_ref() {
                    _info = FileInfo::new(FormatVariant::Csv, compr);
                    delim = Some(',');
                    fields = f.clone();
                } else if let Some(f) = opts.tsv.as_ref() {
                    _info = FileInfo::new(FormatVariant::Tsv, compr);
                    delim = Some('\t');
                    fields = f.clone();
                }

                let format = InFormat::from_opts(
                    _info.format,
                    delim,
                    &fields,
                    opts.header,
                    opts.qual.as_deref(),
                )?;
                let opts = InputOptions::new(kind.clone(), format, _info.compression)
                    .thread_opts(self.advanced.read_thread, None)
                    .reader_opts(None, self.advanced.max_read_mem);
                Ok(opts)
            })
            .collect::<CliResult<_>>()?;
        Ok(input)
    }

    pub fn get_output_opts(&self, informat: Option<&InFormat>) -> CliResult<OutputOptions> {
        let opts = &self.output;
        let (infmt, infields, indelim) = match informat {
            Some(f) => f.decompose(),
            None => (FormatVariant::Fasta, None, None),
        };
        // TODO: ST_FORMAT removed

        // output
        let output = opts.output.clone().unwrap_or(OutputKind::Stdout);

        // get format settings from args
        let mut delim = opts.out_delim;
        let mut fields = opts.outfields.clone();
        let info = opts.to.clone();
        if let Some(i) = info.as_ref() {
            // delimiters need to be defined correctly
            match i.format {
                FormatVariant::Csv => delim = delim.or(Some(',')),
                FormatVariant::Tsv => delim = delim.or(Some('\t')),
                _ => {}
            }
        }

        // if no format specified, infer from path or input format (in that order)
        let mut info = info.unwrap_or_else(|| match &output {
            OutputKind::Stdout => FileInfo::new(infmt.clone(), Compression::None),
            OutputKind::File(path) => FileInfo::from_path(path, infmt.clone()),
        });

        // furthermore, --fa/--fq/--tsv, etc. have highest priority
        let compr = info.compression;
        if opts.to_fa {
            info = FileInfo::new(FormatVariant::Fasta, compr);
        } else if opts.to_fq {
            info = FileInfo::new(FormatVariant::Fastq(QualFormat::Sanger), compr);
        } else if let Some(f) = opts.to_csv.as_ref() {
            info = FileInfo::new(FormatVariant::Csv, compr);
            delim = Some(',');
            fields = Some(f.clone());
        } else if let Some(f) = opts.to_tsv.as_ref() {
            info = FileInfo::new(FormatVariant::Tsv, compr);
            delim = Some('\t');
            fields = Some(f.clone());
        }

        // use input CSV fields and delimiter if not specified otherwise
        let fields = fields
            .or(infields.map(|f| f.join(",")))
            .unwrap_or_else(|| "id,desc,seq".to_string());
        let delim = delim.or(indelim);

        // assemble
        let format = OutFormat::from_opts(
            info.format,
            &self.attr.attr,
            opts.wrap.map(|w| w as usize),
            delim,
            &fields,
            opts.qual_out.as_deref(),
        )?;

        let opts = OutputOptions::new(output, format, info.compression)
            .thread_opts(self.advanced.write_thread, None);
        Ok(opts)
    }

    pub fn get_var_opts(&self) -> CliResult<VarOpts> {
        Ok(VarOpts {
            lists: self.meta.list.clone(),
            list_delim: self.meta.ldelim,
            has_header: self.meta.lheader,
            unordered: self.meta.unordered,
            id_col: self.meta.id_col.checked_sub(1).unwrap(),
            allow_missing: self.meta.missing,
            attr_opts: AttrOpts {
                delim: self.attr.adelim,
                value_delim: self.attr.aval_delim,
            },
            expr_init: self.expr.js_init.clone(),
            var_help: self.general.help_vars,
        })
    }
}

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct ClapCli {
    #[command(subcommand)]
    pub command: SubCommand,
    // #[command(flatten)]
    // pub common: CommonArgs,
}

/// Commands (optional)
#[derive(Subcommand, Clone, Debug)]
pub enum SubCommand {
    /// No processing done, useful for converting and attribute setting
    #[cfg(any(feature = "all_commands", feature = "pass"))]
    #[command(aliases=&["."])]
    Pass(cmd::pass::PassCommand),
    /// Colored sequence view
    #[cfg(any(feature = "all_commands", feature = "view"))]
    View(cmd::view::ViewCommand),
    /// Count sequences (total or by sequence properties)
    #[cfg(any(feature = "all_commands", feature = "count"))]
    Count(cmd::count::CountCommand),
    /// Per-sequence statistics
    #[cfg(any(feature = "all_commands", feature = "stat"))]
    Stat(cmd::stat::StatCommand),

    #[cfg(any(feature = "all_commands", feature = "head"))]
    /// Return the first N sequences
    Head(cmd::head::HeadCommand),
    /// Return the last N sequences
    #[cfg(any(feature = "all_commands", feature = "tail"))]
    Tail(cmd::tail::TailCommand),
    /// Get a slice of the sequences within a defined range
    #[cfg(any(feature = "all_commands", feature = "slice"))]
    Slice(cmd::slice::SliceCommand),
    /// Get a random subset of sequences
    #[cfg(any(feature = "all_commands", feature = "sample"))]
    Sample(cmd::sample::SampleCommand),
    /// Sort records by sequence or any other criterion.
    #[cfg(any(feature = "all_commands", feature = "sort"))]
    Sort(cmd::sort::cli::SortCommand),
    /// De-replicate records, returning only unique ones
    #[cfg(any(feature = "all_commands", feature = "unique"))]
    Unique(cmd::unique::cli::UniqueCommand),
    /// Filter based on different criteria
    #[cfg(any(feature = "all_commands", all(feature = "expr", feature = "filter")))]
    Filter(cmd::filter::FilterCommand),
    /// Distribute sequences into multiple files
    #[cfg(any(feature = "all_commands", feature = "split"))]
    Split(cmd::split::SplitCommand),
    /// Interleave seqs. from multiple files
    #[cfg(any(feature = "all_commands", feature = "interleave"))]
    Interleave(cmd::interleave::InterleaveCommand),
    /// Find one or more patterns with optional filtering/replacement
    #[cfg(any(feature = "all_commands", feature = "find"))]
    Find(cmd::find::FindCommand),
    /// Fast pattern replacement
    #[cfg(any(feature = "all_commands", feature = "replace"))]
    Replace(cmd::replace::ReplaceCommand),
    /// Set a new sequence and/or header
    #[cfg(any(feature = "all_commands", feature = "set"))]
    Set(cmd::set::SetCommand),
    /// Delete description fields and/or attributes
    #[cfg(any(feature = "all_commands", feature = "del"))]
    Del(cmd::del::DelCommand),
    /// Trim sequences on the left and/or right
    #[cfg(any(feature = "all_commands", feature = "trim"))]
    Trim(cmd::trim::TrimCommand),
    /// Soft or hard mask sequence ranges
    #[cfg(any(feature = "all_commands", feature = "mask"))]
    Mask(cmd::mask::MaskCommand),
    /// Convert sequences to uppercase
    #[cfg(any(feature = "all_commands", feature = "upper"))]
    Upper(cmd::upper::UpperCommand),
    /// Convert sequences to lowercase
    #[cfg(any(feature = "all_commands", feature = "lower"))]
    Lower(cmd::lower::LowerCommand),
    /// Reverse complement DNA sequences
    #[cfg(any(feature = "all_commands", feature = "revcomp"))]
    Revcomp(cmd::revcomp::RevcompCommand),
    /// Concatenate seqs. from multiple files
    #[cfg(any(feature = "all_commands", feature = "concat"))]
    Concat(cmd::concat::ConcatCommand),
}

/// Common options
#[derive(Args, Clone, Debug)]
// #[clap(next_help_heading = "Output")]
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
// #[group()]
pub struct GeneralArgs {
    /// Print more detailed information.
    #[arg(short, long)]
    pub verbose: bool,

    /// Display this message
    // #[arg(short, long)]
    // help: bool,

    /// List and explain all available variables
    /// TODO: does not work
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
    /// (can be configured with ST_FORMAT). Possibilities:
    /// fasta (default), fastq (fastq-illumina, fastq-solexa),
    /// csv, tsv or 'fa-qual:<qfile_path>.qual'
    /// Compression: <format>.<compression> (.gz, .bz2 or .lz4).
    /// The csv and tsv variants also accept a comma-separated field list
    /// (instead of --fields). Instead of 'fa-qual', --qfile can be supplied.
    /// Complex combinations possible: --fmt tsv.gz:id:2,desc:6,seq:9
    pub fmt: Option<FileInfo>,

    #[arg(
        long,
        value_name = "FIELDS",
        default_value = "id,desc,seq",
        value_delimiter = ','
    )]
    /// CSV fields: 'id,seq,desc' (in order) or 'id:2,desc:6,seq:9' (col. num.)
    /// or headers: 'id:id,seq:sequence,desc:some_description'
    pub fields: Vec<String>,

    #[arg(long, value_name = "CHAR")]
    /// TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for csv
    pub delim: Option<char>,

    #[arg(long)]
    /// Specify if CSV file has a header. Auto-enabled depending on the format
    /// of --fields, --csv or --tsv
    pub header: bool,

    #[arg(long)]
    /// FASTA input. Short for '--fmt fasta'.
    pub fa: bool,

    #[arg(long)]
    /// FASTQ input. Short for '--fmt fastq'.
    pub fq: bool,

    #[arg(long)]
    /// FASTQ input in Illumina 1.3+ format (alias to --fmt fastq-illumina)
    pub fq_illumina: bool,

    #[arg(long, value_name = "FIELDS", value_delimiter = ',')]
    /// CSV input. Short for '--fmt csv --fields <fields>'
    pub csv: Option<Vec<String>>,

    #[arg(long, value_name = "FIELDS", value_delimiter = ',')]
    /// TSV input. Short for '--fmt tsv --fields <fields>'
    pub tsv: Option<Vec<String>>,

    #[arg(long, value_name = "FILE")]
    /// Path to QUAL file with quality scores (Roche 454 style)
    pub qual: Option<String>,
}

/// Your application's description
#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Output (common to all commands)")]
pub struct OutputArgs {
    #[arg(short, long, value_name = "FILE")]
    /// Write output to <file> instead of STDOUT [Default: STDOUT (-)]
    pub output: Option<OutputKind>,

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

    #[arg(long, value_name = "FIELD")]
    /// Comma delimited list of CSV/TSV fields, which can be
    /// variables/functions or contain variables/expressions.
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
    /// headers (multiple -a key=value args possible).
    /// The default output format is: '>id some description key=value key2=value2'.
    /// To change the format, use --adelim and --aval-delim
    #[arg(short, long, value_name = "KEY=VALUE", action = ArgAction::Append)]
    pub attr: Vec<Attribute>,

    /// Attribute delimiter in the output. If not a space,
    /// attributes are appended to the ID (before the first space)
    /// instead of the description (which comes after the first space).
    #[arg(long, env = "ST_ATTR_DELIM", value_name = "CHAR", default_value = " ")]
    pub adelim: char,

    /// Delimiter between attribute names and values.
    #[arg(
        long,
        env = "ST_ATTRVAL_DELIM",
        value_name = "CHR",
        default_value = "="
    )]
    pub aval_delim: char,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Associated metadata (all commands)")]
pub struct MetaArgs {
    /// Path to list with metadata (multiple -l args possible)
    #[arg(short, long, value_name = "FILE", action = ArgAction::Append)]
    pub list: Vec<String>,

    /// Delimiter for list
    #[arg(long, value_name = "CHAR", default_value = "\t")]
    pub ldelim: char,

    /// List contains a header row. Automatically enabled if
    /// supplying a function with a field name {list_col(fieldname)}.
    #[arg(long)]
    pub lheader: bool,

    /// ID column number
    #[arg(long, value_name = "NUM", default_value_t = 1, value_parser = value_parser!(u32).range(1..))]
    pub id_col: u32,

    /// Allow lists to in different order than sequences.
    #[arg(short, long)]
    pub unordered: bool,

    /// Allow missing rows with '-u'. Variable output is empty.
    #[arg(short, long)]
    pub missing: bool,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Expressions/scripts (all commands)")]
pub struct ExprArgs {
    /// Javascript code to execute during initialization
    /// (for setting global variables used during parsing).
    /// Either a plain string or 'file:path/to/file.js'
    #[arg(long, value_name = "CODE")]
    pub js_init: Option<String>,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Advanced (all commands)")]
pub struct AdvancedArgs {
    /// Buffer size limit for the internal reader. Larger sequence records will
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
}
