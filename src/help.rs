macro_rules! common_opts { () => (r"
Input options:
    --fmt <format>      Input format: fasta(default), fastq (fastq-illumina,
                        fastq-solexa), or csv / tsv (=txt).
                        Compression: <format>.<compression> (.gz, .bz2 or .lz4).
                        Only needed if format cannot be guessed from extension.
    --fields <fields>   CSV fields: 'id,seq,desc' (in order) or 'id:2,desc:6,seq:9'
                        (col. num.) or headers: 'id:id,seq:sequence,desc:desc'
                        [default: id,seq,desc]
    --delim <delim>     TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for csv
    --header            Specify if CSV file has a header. Auto-enabled with headers.
    --fa                FASTA input. Short for '--fmt fasta'.
    --fq                FASTQ input. Short for '--fmt fastq'.
    --fq-illumina       FASTQ input in Illumina 1.3+ format (--fmt fastq-illumina)
    --csv <fields>      CSV input. Short for '--fmt csv --fields <fields>'
    --tsv <fields>      TSV input. Short for '--fmt tsv --fields <fields>'
    --qual <file>       Path to QUAL file with quality scores (Roche 454 style)

Output options:
    -o, --output <f>    Write output to <file> instead of STDOUT [default: -].
    --to <outformat>    Output format and compression. See --fmt. Only needed
                        if not guessed from the extension (default: input format).
    --wrap <width>      Wrap FASTA sequences to maximum <width> characters
    --out-delim <d>     TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for csv
    --outfields <f>     TSV/CSV fields (variables allowed). [default: id,seq,desc]
    --to-fa             FASTA output. Short for: '--to fasta'
    --to-fq             FASTQ output. Short for: '--to fastq'
    --to-csv <fields>   CSV output. Short for '--to csv --outfields <f>'
    --to-tsv <fields>   TSV output. Short for '--to tsv --outfields <f>'
    --compr-level <l>   Level for compressed output. 1-9 for GZIP/BZIP2 and
                        1-21 for ZSTANDARD
    --qual-out <file>   Path to QUAL output file with quality scores

Attribute options:
    -a, --attr <a>      Add an attribute in the form name=value to FASTA/FASTQ
                        headers (multiple '-a key=value' args possible)
    --adelim <delim>    Attribute delimiter inserted before. If not a space,
                        attributes are appended to the ID (default: ' ')
    --aval-delim <d>    Delimiter between attribute names and values [default: =]

Associated lists:
    -l, --list <path>   Path to list with metadata (multiple -l args possible)
    --ldelim <delim>    Delimiter for list [default: \t]
    --lheader           List contains a header row. Automatically enabled if
                        variables in the form {l:<name>} are found.
    --id-col <no>       ID column number [default: 1]
    -u, --unordered     Allow lists to in different order than sequences.
    -m, --missing       Allow missing rows with '-u'. Variable output is empty.

General Information:
    -v, --verbose       Print more detailed information.
    -h, --help          Display this message
    --help-vars         List and explain all available variables

Advanced Options:
    --buf-cap <size>    Initial capacity of internal reader buffer [default: 68K]
    --max-mem <size>    Buffer size limit. Larger sequences will cause an error.
                        [default: 1G]
    -T, --read-thread   Read from a different thread. Enabled with compressed input.
    --write-thread      Write in a different thread. Enabled with compressed output.
    --read-tbufsize S   Buffer size of threaded reader (default: auto)
    --write-tbufsize S  Buffer size of threaded reader (default: auto)
")}


macro_rules! command_list { () => ("
    pass        No processing done, useful for converting and attribute setting
    .           shorthand for 'pass'

Information about sequences
    view        Colored sequence view
    count       Returns the sequence count
    stat        Per-sequence statistics

Subsetting / shuffling sequences
    head        Return the first N sequences
    tail        Return the last N sequences
    slice       Get a slice of the sequences within a defined range
    sample      Get a random subset of sequences
    filter      Filter based on different criteria
    split       Distribute sequences into multiple files
    interleave  Interleave seqs. from multiple files

Searching and replacing
    find        Find one or more patterns with optional filtering/replacement
    replace     Fast pattern replacement

Modifying commands
    set         Set a new sequence and/or header
    del         Delete description fields and/or attributes
    trim        Trim sequences on the left and/or right
    mask        Soft or hard mask sequence ranges
    upper       Convert sequences to uppercase
    lower       Convert sequences to lowercase (soft mask)
    revcomp     Reverse complement DNA sequences
    concat      Concatenate seqs. from multiple files

For information about how to use a command use
    seqtool <command> -h/--help

List and explain available variables:
    seqtool --help-vars
    seqtool <command> --help-vars

")}

pub static USAGE: &'static str = concat!("
Tool for processing of biological sequences. It can read and write the formats
FASTA, FASTQ and CSV/TSV.

Usage:
    seqtool <command> [<opts>...]
    seqtool <command> (-h | --help)
    seqtool <command> --help-vars
    seqtool [options]

Options:
    -h, --help    Display this message
    --version     Print version info and exit
    --help-vars   Display variables accepted by all commands

Commands:
", command_list!());
