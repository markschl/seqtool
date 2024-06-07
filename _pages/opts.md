

### Options recognized by all commands

```
General options (common to all commands):
  -v, --verbose    Print more detailed information about the progress and
                   results of certain commands
  -q, --quiet      Suppress all messages except errors and important warnings
      --help-vars  List and explain all variables/functions available

Input (common to all commands):
      --fmt <FMT>        Input format, only needed if it cannot be guessed from
                         the extension (e.g. if reading from STDIN). 'fasta' is
                         assumed as default (can be configured with ST_FORMAT).
                         Possible choices: fasta (default), fastq
                         (fastq-illumina, fastq-solexa), csv or tsv Compression:
                         <format>.<compression> (.gz, .bz2 or .lz4) [env:
                         ST_FORMAT=]
      --fa               FASTA input. Short for '--fmt fasta'
      --fq               FASTQ input. Short for '--fmt fastq'
      --fq-illumina      FASTQ input in Illumina 1.3-1.7 format (alias to --fmt
                         fastq-illumina)
      --fields <FIELDS>  CSV fields: 'id,seq,desc' (in order) or
                         'id:2,desc:6,seq:9' (col. num.) or headers:
                         'id:id,seq:sequence,desc:some_description' [default:
                         id,desc,seq]
      --delim <CHAR>     TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ',' for
                         csv
      --header           Specify if CSV file has a header. Auto-enabled
                         depending on the format of --fields, --csv or --tsv
      --csv <FIELDS>     CSV input. Short for '--fmt csv --fields <fields>'
      --tsv <FIELDS>     TSV input. Short for '--fmt tsv --fields <fields>'
      --qual <FILE>      Path to QUAL file with quality scores (Roche 454 style)
      --seqtype <TYPE>   Sequence type; relevant for the `find` and `revcomp`
                         commands, as well as the variables/functions
                         `seq_revcomp`, `seqhash_rev` and `seqhash_both`
                         (default: auto-detected based on the first sequence)
                         [possible values: dna, rna, protein, other]
  [INPUT]...         Input file(s), multiple possible (use '-' for STDIN)
                     [default: -]

Output (common to all commands):
  -o, --output <FILE>       Write output to <file> instead of STDOUT [Default:
                            STDOUT (-)]
      --to <FORMAT>         Output format and compression. See --fmt. Only
                            needed if not guessed from the extension (default:
                            input format)
      --wrap <WIDTH>        Wrap FASTA sequences to maximum <width> characters
      --out-delim <DELIM>   TSV/CSV delimiter. Defaults: '\t' for tsv/txt; ','
                            for csv
      --outfields <FIELDS>  Comma delimited list of CSV/TSV fields, which can be
                            variables/functions or contain
                            variables/expressions. [default: input fields or
                            'id,desc,seq']
      --to-fa               FASTA output. Short for: '--to fasta'
      --to-fq               FASTQ output. Short for: '--to fastq'
      --to-csv <FIELDS>     CSV output with comma delimited list of fields,
                            which can be variables/functions or contain
                            variables/expressions. Short for '--to csv
                            --outfields <f>'
      --to-tsv <FIELDS>     TSV output with comma delimited list of fields,
                            which can be variables/functions or contain
                            variables/expressions. Short for '--to tsv
                            --outfields <f>'
      --compr-level <L>     Level for compressed output. 1-9 for GZIP/BZIP2
                            (default=6) and 1-16 for LZ4 (default=0). 1-22 for
                            Zstandard (default=3 or 0)
      --qual-out <FILE>     Path to QUAL output file with quality scores

FASTA/Q header attributes (all commands):
  -a, --attr <KEY=VALUE>   Add an attribute in the form name=value to
                           FASTA/FASTQ headers or replace their value if the
                           given name already exists (multiple -a key=value
                           arguments possible). The default output format is:
                           '>id some description key1=value1 key2=value2'. Use
                           --attr-format to change
  -A, --attr-append <K=V>  Append one or multiple attributes in the form
                           name=value to FASTA/FASTQ headers. Compared to
                           `-a/--attr`, existing attributes in headers are NOT
                           replaced. This will result in a duplicate entry if
                           the given attribute name already exists
      --attr-fmt <FMT>     Expected format of sequence header attributes, which
                           is also used for writing new attributes to headers
                           (using -a/--attr). The words 'key' and 'value' must
                           always be present, and 'value' must follow after
                           'key'. Example: ';key=value'. If the delimiter before
                           the key is not a space attributes are appended to the
                           ID (part before the first space) instead of the end
                           of the header [env: ST_ATTR_FORMAT=] [default: "
                           key=value"]

Associated metadata (all commands):
  -m, --meta <FILE>        Delimited text file path (or '-' for STDIN)
                           containing associated metadata, accessed using the
                           `meta(field)` function, or `meta(field, file-num)` in
                           case of multiple metadata files (supplied like this:
                           -m file1 -m file2 ...)
      --meta-delim <CHAR>  Metadata column delimiter. Inferred from the file
                           extension if possible: '.csv' is interpreted as
                           comma(,)-delimited, '.tsv'/'.txt' or other (unknown)
                           extensions are assumed to be tab-delimited [default:
                           "\t"]
      --meta-header        Specify if the first row of the metadata file(s)
                           contains column names. Automatically enabled if a
                           non-numeric field names are used, e.g.
                           'meta(fieldname)'
      --meta-idcol <NUM>   Column number containing the sequence record IDs
                           [default: 1]
      --dup-ids            Specify if the sequence input is expected to contain
                           duplicate IDs. Without this flag, there may be an
                           error (`meta` and `has_meta` functions), whereas
                           `opt_meta` may wrongly return missing values

Expressions/scripts (all commands):
      --js-init <CODE>  Javascript code to execute during initialization (e.g.
                        for defining global variables used later during
                        parsing). Either a plain string or
                        'file:path/to/file.js'

Advanced (all commands):
      --max-read-mem <SIZE>  Buffer size limit for the internal reader. Larger
                             sequence records will cause an error. Note, that
                             some commands such as 'sort', 'unique' and 'sample'
                             still use more memory and have their own additional
                             memory limit setting. Either a plain number (bytes)
                             a number with unit (K, M, G, T) based on powers of
                             2 [default: 1G]
  -T, --read-thread          Read from a different thread. Enabled with
                             compressed input
  -W, --write-thread         Write in a different thread. Enabled with
                             compressed output
```
