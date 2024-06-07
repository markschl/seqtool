Directly pass input to output without any processing, useful for converting and
attribute setting

```
Usage: st pass [OPTIONS] [INPUT]...

Options:
  -h, --help  Print help
```

[See this page](opts) for the options common to all commands.

## Contents

* [Examples](#examples)
* [Recognized formats](#recognized-formats)
* [Delimited formats (CSV, TSV, ...)](#delimited-formats-csv,-tsv,-...)
* [Setting default via environment variable](#setting-default-via-environment-variable)
* [Quality scores](#quality-scores)

## Details
Simple conversions can be done using the `pass` command, although every
other command supports all input/output formats (except for the statistical
commands, which don't return any sequences).

### Examples

```sh
st pass input.fastq.gz -o output.fasta
```

equivalent, shorter notation:

```sh
st . input.fastq.gz -o output.fasta
```

The input and output formats are automatically inferred based on the file
extensions, assuming GZIP compressed FASTQ input and FASTA output.
If receiving from STDIN or writing to STDOUT, the format has to be
specified unless it is FASTA, which is the default.

```sh
cat input.fastq.gz | st . --fmt fastq.gz --to fasta > output.fasta
```
Note that GZIP compression can be specified in the format string by adding
`.gz`.
The output format is always assumed to be the same as the input format
if not specified otherwise by using `--to <format>` or `-o <path>.<extension>`.
Writing `--to tsv --outfields id,seq` is quite verbose, therefore
a shortcut exists: `--to-tsv id,seq`. Similar shortcuts are avialable for uncompressed
input/output in other formats.


### Recognized formats

The following extensions and format strings are auto-recognized:

sequence format      | recognized extensions | format string | shortcut (in) | ..out
-------------------- | --------------------- | ------------- | ------------- | ----------
FASTA                |  `.fasta`,`.fa`,`.fna`,`.fsa`| `fasta`,`fa`| `--fa`        | `--to-fa`
FASTQ                |  `.fastq`,`.fq`       | `fastq`,`fq`,`fq—illumina`,`fq—solexa`| `--fq`| `--to—fq`
CSV (`,` delimited)  |  `.csv`               | `csv`         | `--csv FIELDS`| `--to—csv FIELDS`
TSV (`tab` delimited)|  `.tsv`,`.tsv`        | `tsv`         | `--tsv FIELDS`| `--to—tsv FIELDS `

**Note:** Multiline FASTA is parsed and written (`--wrap`), but only single-line
FASTQ is parsed and written.

Quality scores can also be parsed from and written to 454 (Roche) style `QUAL`
files using `--qual <file>` and `--to-qual <file>`.

Compression formats (no shortcuts available, use `--fmt <input_format>` / `--to <output_format>`):

format       | recognized extensions | format string (FASTA)
------------ | --------------------- | ---------------------
GZIP         |  `.gzip`,`.gz`        | `fasta.gz`
BZIP2        |  `.bzip2`,`.bz2`      | `fasta.bz2`
LZ4          |  `.lz4`               | `fasta.lz4`
ZSTD         |  `.zst`               | `fasta.zst`


### Delimited formats (CSV, TSV, ...)

Comma / tab / ... delimited input and output can be configured providing the
`--fields` / `--outfields` argument, or directly using `--csv`/`--to-csv`
or `--tsv`/`--to-tsv`. The delimiter is configured with `--delim <delim>`

```sh
st . --outfields id,seq -o output.tsv input.fasta
```

equivalent shortcut:

```sh
st . --to-tsv id,seq > output.tsv
```

[Variables](variables) can also be included:

```sh
st . --to-tsv "id,seq,length: {s:seqlen}" input.fasta
```

returns:

```
id1	ATGC(...)	length: 231
id2	TTGC(...)	length: 250
```

### Setting default via environment variable

The `ST_FORMAT` environment variable can be used to set a default format other
than FASTA. This is especially useful if connecting many commands via pipe,
saving the need to specify `--fq` / `--tsv <fields>` / ... repeatedly. Example:

```sh
export ST_FORMAT=fastq

st trim :10 input.fastq | st revcomp > trimmed_revcomp.fastq
```

For delimited files (CSV or TSV), the input fields can be configured
additionally after a colon (':'):

```sh
export ST_FORMAT=tsv:id,seq

## Input file:
# id1 ACGT...
# id2 ACGT...
# ...

st trim ':4' input.txt | st revcomp > trimmed_revcomp.txt

## Output:
# id1 ACGT...
# id2 ACGT...
#...
```

### Quality scores

Quality scores can be read from several sources.
[FASTQ](https://en.wikipedia.org/wiki/FASTQ_format) files are assumed to be
in the Sanger / Illumina 1.8+ format. Older formats (Illumina 1.3+ and Solexa)
can be read and written using `--fmt/--to fq-illumina` or `fq-solexa`. Automatic
unambiguous recognition of the formats is not possible, therefore the formats have
to be explicitly specified. Invalid characters generate an error during conversion.
If no conversion is done (e.g. both input and output in Sanger/Illumina 1.8+ format),
scores are not automatically checked for errors.

Quality scores can be visualized using the [view command](view).

The following example converts a legacy Illumina 1.3+ file to the Sanger /
Illumina 1.8+ format:

```sh
st . --fmt fq-illumina --to.fastq illumina_1_3.fastq > sanger.fastq
```
Another useful application is filtering by quality (see [filter command](filter#quality-filtering)).
