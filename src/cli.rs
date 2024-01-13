use std::process::exit;

use crate::cmd;
use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::bytesize::parse_bytesize;
use crate::io::{
    input::{InFormat, InputKind, InputOptions},
    output::{OutFormat, OutputKind, OutputOptions},
    Attribute, Compression, FileInfo, FormatVariant, QualFormat,
};
use crate::var::{AttrOpts, VarHelp, VarOpts};

use clap::{value_parser, ArgAction, Args, Parser, Subcommand};

/// This type only serves as a workaround to allow displaying
/// custom help page that explains all variables (--help-vars)
/// The actual CLI interface is defined by `ClapCli`
#[derive(Parser, Debug, Clone)]
struct VarHelpCli {
    command: String,

    #[arg(long)]
    help_vars: bool,
}

#[derive(Debug, Clone)]
pub struct Cli(ClapCli);

impl Cli {
    pub fn new() -> Self {
        // first, try to look for --help-vars using the extra Clap parser
        // in order to work around the fact that clap exits with a
        // 'missing argument error' for command with positional args.
        // TODO: any better way to do this?
        if let Ok(m) = VarHelpCli::try_parse() {
            if m.help_vars {
                let custom_help: Option<Box<dyn VarHelp>> = match m.command.as_str() {
                    #[cfg(any(feature = "all_commands", feature = "sort", feature = "unique"))]
                    "sort" | "unique" => Some(Box::new(cmd::shared::key_var::KeyVarHelp)),
                    #[cfg(any(feature = "all_commands", feature = "split"))]
                    "split" => Some(Box::new(cmd::split::ChunkVarHelp)),
                    #[cfg(any(feature = "all_commands", feature = "find"))]
                    "find" => Some(Box::new(cmd::find::FindVarHelp)),
                    _ => None,
                };
                eprintln!("{}", crate::var::get_var_help(custom_help).unwrap());
                exit(2);
            }
        }
        Self(ClapCli::parse())
    }

    pub fn run(&mut self) -> CliResult<()> {
        use SubCommand::*;
        macro_rules! run {
            ($cmdmod:ident, $opts:expr) => {
                run!($cmdmod, $opts, None)
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
            Sort(ref opts) => run!(sort, opts, cmd::sort::get_varprovider(opts)),
            #[cfg(any(feature = "all_commands", feature = "unique"))]
            Unique(ref opts) => run!(unique, opts, cmd::unique::get_varprovider(opts)),
            #[cfg(any(feature = "all_commands", all(feature = "expr", feature = "filter")))]
            Filter(ref opts) => run!(filter, opts),
            #[cfg(any(feature = "all_commands", feature = "split"))]
            Split(ref opts) => run!(split, opts, cmd::split::get_varprovider(opts)),
            #[cfg(any(feature = "all_commands", feature = "interleave"))]
            Interleave(ref opts) => run!(interleave, opts),
            #[cfg(any(feature = "all_commands", feature = "find"))]
            Find(ref opts) => run!(find, opts, cmd::find::get_varprovider(opts)),
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
                    InputKind::File(path) => FileInfo::from_path(path, FormatVariant::Fasta, true),
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
                    .thread_opts(self.advanced.read_thread, self.advanced.read_tbufsize)
                    .reader_opts(self.advanced.buf_cap, self.advanced.max_read_mem);
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
            OutputKind::File(path) => FileInfo::from_path(path, infmt.clone(), true),
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
            .thread_opts(self.advanced.write_thread, self.advanced.write_tbufsize);
        Ok(opts)
    }

    pub fn get_var_opts(&self) -> CliResult<VarOpts> {
        Ok(VarOpts {
            metadata_sources: self.meta.meta.clone(),
            meta_delim_override: self.meta.meta_delim.map(|d| d as u8),
            has_header: self.meta.meta_header,
            meta_id_col: self.meta.meta_idcol.checked_sub(1).unwrap(),
            meta_dup_ids: self.meta.dup_ids,
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
#[command(author, version, about)]
pub struct ClapCli {
    #[command(subcommand)]
    pub command: SubCommand,
}

/// Commands (optional)
#[derive(Subcommand, Clone, Debug)]
pub enum SubCommand {
    /// No processing done, useful for converting and attribute setting
    #[cfg(any(feature = "all_commands", feature = "pass"))]
    #[command(visible_aliases=&["."])]
    Pass(cmd::pass::PassCommand),
    /// Colored sequence view
    ///
    /// View biological sequences, colored by base / amino acid, or by sequence quality.
    /// The output is automatically forwarded to the 'less' pager on UNIX.
    #[cfg(any(feature = "all_commands", feature = "view"))]
    View(cmd::view::ViewCommand),
    /// Count sequences (total or by sequence properties)
    ///
    /// This command counts the number of sequences in total or per category.
    /// Results are printed to STDOUT.
    /// Advanced grouping of sequences is possible by supplying or more key strings
    /// containing variables (-k/--key).
    #[cfg(any(feature = "all_commands", feature = "count"))]
    Count(cmd::count::CountCommand),
    /// Per-sequence statistics as tab-delimited list
    ///
    /// Returns per sequence statistics as tab delimited list. All variables
    /// (seqlen, exp_err, charcount(...), etc.) can be used (see `st stat --help-vars`).
    /// The command is equivalent to `st pass --to-tsv 'id,var1,var2,...' input`
    #[cfg(any(feature = "all_commands", feature = "stat"))]
    Stat(cmd::stat::StatCommand),
    /// Return the first N sequences
    #[cfg(any(feature = "all_commands", feature = "head"))]
    Head(cmd::head::HeadCommand),
    /// Return the last N sequences
    ///
    /// This only works for files (not STDIN), since records are counted in a first
    /// step, and only returned after reading a second time.
    #[cfg(any(feature = "all_commands", feature = "tail"))]
    Tail(cmd::tail::TailCommand),
    /// Return a range of sequence records from the input
    ///
    /// The range is specified as start..end, whereby start and end
    /// are the sequence numbers (starting from 1). Open ranges are
    /// possible. The following commmands are equivalent with the
    /// 'head' and 'tail' commands:
    ///
    /// > st slice ..10 input.fasta
    ///
    /// > st slice '-10..' input.fasta
    ///
    /// This command does not extract subsequences (see trim command for that).
    #[cfg(any(feature = "all_commands", feature = "slice"))]
    Slice(cmd::slice::SliceCommand),
    /// Get a random subset of sequences
    ///
    /// Either a fixed number (-n/--num-seqs) or an approximate fraction of the
    /// input (-p/--prob) can be sampled (see help for these options).
    /// The records are returned in input order.
    #[cfg(any(feature = "all_commands", feature = "sample"))]
    Sample(cmd::sample::SampleCommand),
    /// Sort records by sequence or any other criterion.
    ///
    /// The sort key can be 'seq', 'id', or any variable/function, expression, or
    /// text containing them (see <KEY> help).
    ///
    /// Records are sorted in memory, it is up to the user of this function
    /// to ensure that the whole input will fit into memory.
    /// The default sort is by sequence.
    ///
    /// The actual value of the key is available through the 'key' variable. It can
    /// be written to a header attribute or TSV field.
    /// This may be useful with JavaScript expressions, whose evaluation takes time,
    /// and whose result should be written to the headers, e.g.:
    /// 'st sort -n '{{ id.substring(3, 5) }}' -a id_num='{key}' input.fasta'
    #[cfg(any(feature = "all_commands", feature = "sort"))]
    Sort(cmd::sort::cli::SortCommand),
    /// De-replicate records by sequence or any other criterion, returning only
    /// unique records.
    ///
    /// The unique key can be 'seq' or any variable/function, expression, or
    /// text containing them (see <KEY> help).
    ///
    /// The order of the records is the same as in the input unless the memory limit
    /// is exceeded, in which case temporary files are used and the records are
    /// sorted by the unique key. Specify -s/--sorted to always sort the output.
    #[cfg(any(feature = "all_commands", feature = "unique"))]
    #[clap(about, long_about)]
    Unique(cmd::unique::cli::UniqueCommand),
    /// Filter sequences using Javascript expressions
    #[cfg(any(feature = "all_commands", all(feature = "expr", feature = "filter")))]
    Filter(cmd::filter::FilterCommand),
    /// Distribute sequences into multiple files based on variable/function(s)
    /// or advanced expressions.
    ///
    /// In contrast to other commands, the output (-o) argument can
    /// contain variables in order to determine the file path for each sequence.
    #[cfg(any(feature = "all_commands", feature = "split"))]
    Split(cmd::split::SplitCommand),
    /// Interleave records of all files in the input.
    ///
    /// The records will returned in the same order as in the input files.
    #[cfg(any(feature = "all_commands", feature = "interleave"))]
    Interleave(cmd::interleave::InterleaveCommand),
    /// Search for one or more patterns in sequences or headers for
    /// record filtering, pattern replacement or writing matches to the output.
    #[cfg(any(feature = "all_commands", feature = "find"))]
    Find(cmd::find::FindCommand),
    /// Fast and simple pattern replacement in sequences or sequence headers
    #[cfg(any(feature = "all_commands", feature = "replace"))]
    Replace(cmd::replace::ReplaceCommand),
    /// Replace the header, header attributes or sequence with new content
    #[cfg(any(feature = "all_commands", feature = "set"))]
    Set(cmd::set::SetCommand),
    /// Delete header ID/description and/or attributes
    #[cfg(any(feature = "all_commands", feature = "del"))]
    Del(cmd::del::DelCommand),
    /// Trim sequences on the left and/or right (single range)
    /// or extract and concatenate several subsequences.
    #[cfg(any(feature = "all_commands", feature = "trim"))]
    Trim(cmd::trim::TrimCommand),
    /// Soft or hard mask sequence ranges
    ///
    /// Masks the sequence within a given range or comma delimited list of ranges
    /// by converting to lowercase (soft mask) or replacing with a character (hard
    /// masking). Reverting soft masking is also possible.
    #[cfg(any(feature = "all_commands", feature = "mask"))]
    Mask(cmd::mask::MaskCommand),
    /// Convert sequences to uppercase
    #[cfg(any(feature = "all_commands", feature = "upper"))]
    Upper(cmd::upper::UpperCommand),
    /// Convert sequences to lowercase
    #[cfg(any(feature = "all_commands", feature = "lower"))]
    Lower(cmd::lower::LowerCommand),
    /// Reverse complements DNA sequences. If quality scores are present,
    /// their order is just reversed.
    #[cfg(any(feature = "all_commands", feature = "revcomp"))]
    Revcomp(cmd::revcomp::RevcompCommand),
    /// Concatenates sequences/alignments from different files in the order
    /// in which they are provided.
    ///
    /// Fails if the sequence IDs don't match.
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
    /// csv or tsv
    /// Compression: <format>.<compression> (.gz, .bz2 or .lz4).
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
    /// FASTQ input in Illumina 1.3-1.7 format (alias to --fmt fastq-illumina)
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
    /// The default output format is: '>id some description key1=value1 key2=value2'.
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
    /// Delimited text file path (or '-' for STDIN) containing associated metadata,
    /// accessed using the `meta(field)` function, or `meta(file-num, field)` in case
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
    /// (for setting global variables used during parsing).
    /// Either a plain string or 'file:path/to/file.js'
    #[arg(long, value_name = "CODE")]
    pub js_init: Option<String>,
}

#[derive(Args, Clone, Debug)]
#[clap(next_help_heading = "Advanced (all commands)")]
pub struct AdvancedArgs {
    /// Initial capacity of internal reader buffer. Either a plain number (bytes)
    /// a number with unit (K, M, G, T) based on powers of 2.
    #[arg(long, value_name = "SIZE", value_parser = parse_bytesize, default_value = "64K", hide = true)]
    pub buf_cap: usize,

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
