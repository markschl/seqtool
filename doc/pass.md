Simple conversions can be done using the `pass` command, although every
other command supports all input/output formats (except for the statistical
commands, which don't return any sequences).

```bash
seqtool pass input.fastq.gz -o output.fasta
# equivalent, shorter notation:
seqtool . input.fastq.gz -o output.fasta
```
The input and output formats are automatically inferred based on the file
extensions, assuming GZIP compressed FASTQ input and FASTA output.
If receiving from STDIN or writing to STDOUT, the format has to be
specified unless it is FASTA, which is the default.

```bash
cat input.fastq.gz | seqtool . --format fastq.gz --outformat fasta > output.fasta
```
Note that GZIP compression can be specified in the format string by adding
`.gz`.
The output format is always assumed to be the same as the input format
if not specified otherwise by using `--outformat <format>` or `-o <path>`.
Writing `--outformat txt --outfields id,seq` is quite verbose, therefore
a shortcut exists: `--to-txt id,seq`. Similar shortcuts are avialable for uncompressed
input/output in other formats.

The following extensions and format strings are auto-recognized:

sequence format      | recognized extensions | format string | shortcut (in) | ..out
-------------------- | --------------------- | ------------- | ------------- | ----------
FASTA                |  `.fasta`,`.fa`,`.fna`,`.fsa`| `fasta`       | `--fa`        | `--to-fa`
FASTQ                |  `.fastq`,`.fq`       | `fastq`       | `--fq`        | `--to-fq`
CSV (`,` delimited)  |  `.csv`               | `csv`         | `--csv FIELDS`| `--to-csv FIELDS`
TXT (`tab` delimited)|  `.txt`               | `txt`         | `--txt FIELDS`| `--to-txt FIELDS `

**Note:** Multiline FASTA is parsed and written (`--wrap`), but only single-line
FASTQ is parsed and written.

Compression formats (no shortcuts available, use `--[out]format`):

format       | recognized extensions | format string (FASTA)
------------ | --------------------- | ---------------------
GZIP         |  `.gzip`,`.gz`        | `fasta.gz`
BZIP2        |  `.bzip2`,`.bz2`      | `fasta.bz2`
LZ4          |  `.lz4`               | `fasta.lz4`

#### CSV / TXT files

Comma / tab delimited input and output can be configured providing the
`--fields` / `--outfields` argument, or directly using `--csv`/`--to-csv`
or `--txt`/`--to-txt`.

```bash
seqtool . --outfields id,seq -o output.txt input.fa

# equivalent shortcut:
seqtool . --to-txt id,seq > output.txt
```

[Variables](variables) can also be included:

```bash
seqtool . --to-txt 'id,seq,length: {s:seqlen}' input.fa
```

returns:

```
id1	ATGC(...)	length: 231
id2	TTGC(...)	length: 250
```
