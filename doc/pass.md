Simple conversions can be done using the `pass` command, although every
other command supports all input/output formats (except for the statistical
commands, which don't return any sequences).

#### Examples

```bash
st pass input.fastq.gz -o output.fasta
# equivalent, shorter notation:
st . input.fastq.gz -o output.fasta
```
The input and output formats are automatically inferred based on the file
extensions, assuming GZIP compressed FASTQ input and FASTA output.
If receiving from STDIN or writing to STDOUT, the format has to be
specified unless it is FASTA, which is the default.

```bash
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
FASTQ                |  `.fastq`,`.fq`       | `fastq`,`fq`,`fq-illumina`,`fq-solexa`| `--fq`        | `--to-fq`
CSV (`,` delimited)  |  `.csv`               | `csv`         | `--csv FIELDS`| `--to-csv FIELDS`
TSV (`tab` delimited)|  `.tsv`,`.tsv`        | `tsv`         | `--tsv FIELDS`| `--to-tsv FIELDS `

**Note:** Multiline FASTA is parsed and written (`--wrap`), but only single-line
FASTQ is parsed and written.

Quality scores can also be parsed from and written to 454 (Roche) style `QUAL`
files using `--qual <file>` and `--to-qual <file>`.

Compression formats (no shortcuts available, use `--[out]format`):

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

```bash
st . --outfields id,seq -o output.tsv input.fa

# equivalent shortcut:
st . --to-tsv id,seq > output.tsv
```

[Variables](variables) can also be included:

```bash
st . --to-tsv "id,seq,length: {s:seqlen}" input.fa
```

returns:

```
id1	ATGC(...)	length: 231
id2	TTGC(...)	length: 250
```

### Quality scores

Quality scores can be read from several sources.
[FASTQ](https://en.wikipedia.org/wiki/FASTQ_format) are assumed to be
in the Sanger / Illumina 1.8+ format. Older formats (Illumina 1.3+ and Solexa)
can be read, but must be specifically specified. The scores can be
visualized using the [view command](view).

The following example converts a legacy Illumina 1.3+ file to the Sanger /
Illumina 1.8+ format:

```bash
st . --fmt fq-illumina --to fq illumina_1_3.fq > sanger.fq
```
Another useful application is filtering by quality (see [filter command](filter#quality-filtering)).
