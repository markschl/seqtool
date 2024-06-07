
# Variables/functions: full reference

## Contents

* [General properties of sequence records and input files](#general-properties-of-sequence-records-and-input-files)
* [Sequence statistics](#sequence-statistics)
* [Header attributes](#header-attributes)
* [Access metadata from delimited text files](#access-metadata-from-delimited-text-files)
* [Expressions (JavaScript)](#expressions-javascript)
* [Data conversion and transformation](#data-conversion-and-transformation)

## General properties of sequence records and input files
 

| | |
|-|-|
| `id` | Record ID (in FASTA/FASTQ: everything before first space) |
| `desc` | Record description (everything after first space) |
| `seq` | Record sequence |
| `upper_seq` | Record sequence in uppercase letters |
| `lower_seq` | Record sequence in lowercase letters |
| `seqhash`<br />`seqhash(ignorecase)` | Calculates a hash value from the sequence using the XXH3 algorithm. A hash is a integer number representing the sequence. In very rare cases, different sequences may lead to the same hash value, but using 'seqhash' instead of 'seq' speeds up de-replication ('unique' command) and requires less memory, at a very small risk of wrongly recognizing two different sequences as duplicates. The returned numbers can be positive or negative. |
| `seqhash_rev`<br />`seqhash_rev(ignorecase)` | The hash value of the reverse-complemented sequence |
| `seqhash_both`<br />`seqhash_both(ignorecase)` | The sum of the hashes from the forward and reverse sequences. The result is always the same irrespective of the sequence orientation, which is useful when de-replicating sequences with potentially different orientations. [side note: to be precise it is a *wrapping addition* to prevent integer overflow] |
| `seq_num`<br />`seq_num(reset)` | Sequence number (n-th sequence in the input), starting from 1. The numbering continues across all provided sequence files unless `reset` is `true`, in which case the numbering re-starts from 1 for each new sequence file.\<br /\>Note that the output order can vary with multithreaded processing. |
| `seq_idx`<br />`seq_idx(reset)` | Sequence index, starting from 0.\<br /\>The index is incremented across all provided sequence files unless `reset` is `true`, in which case the index is reset to 0 at the start of each new sequence file.\<br /\>Note that the output order can vary with multithreaded processing. |
| `path` | Path to the current input file (or '-' if reading from STDIN) |
| `filename` | Name of the current input file with extension (or '-') |
| `filestem` | Name of the current input file *without* extension (or '-') |
| `extension` | Extension of the current input file (or '') |
| `dirname` | Name of the base directory of the current file (or '') |
| `default_ext` | Default file extension for the configured output format (e.g. 'fasta' or 'fastq') |
### Examples
Add the sequence number to the ID:
```sh
st set -i {id}_{seq_num}
```
```
>A_1
SEQUENCE
>B_2
SEQUENCE
>C_3
SEQUENCE
(...)
```
Count the number of records per file in the input:
```sh
st count -k path *.fasta
```
```
file1.fasta	1224818
file2.fasta	573
file3.fasta	99186
(...)
```
Remove records with duplicate sequences from the input:
```sh
st unique seq input.fasta
```
Remove duplicate records irrespective of the sequence orientation and whether letters are uppercase or lowercase:
```sh
st unique 'seqhash_both(true)' input.fasta
```
## Sequence statistics
 

| | |
|-|-|
| `seqlen` | Sequence length |
| `ungapped_seqlen` | Ungapped sequence length (without gap characters `-`) |
| `gc` | GC content as fraction (0-1) of total bases. Lowercase (=masked) letters or characters other than ACGTU are not taken into account. |
| `gc_percent` | GC content as percentage of total bases. Lowercase (=masked) letters or characters other than ACGTU are not taken into account. |
| `charcount(characters)` | Count the occurrences of one or more single characters, which are supplied as a string |
| `exp_err` | Total number of errors expected in the sequence, calculated from the quality scores as the sum of all error probabilities. For FASTQ, make sure to specify the correct format (--fmt) in case the scores are not in the Sanger/Illumina 1.8+ format. |
### Examples
List the GC content (in %) for every sequence:
```sh
st stat gc_percent input.fa
```
```
seq1	33.3333
seq2	47.2652
seq3	47.3684
```
Remove DNA sequences with more than 1% ambiguous bases:
```sh
st filter 'charcount("ACGT") / seqlen >= 0.99' input.fa
```
## Header attributes
Attributes stored in FASTA/FASTQ headers. The expected pattern is ' key=value', but other patterns can be specified with `--attr-format`.

| | |
|-|-|
| `attr(name)` | Obtain an attribute of given name (must be present in all sequences) |
| `opt_attr(name)` | Obtain an attribute value, or 'undefined' if missing (=undefined in JavaScript expressions) |
| `attr_del(name)` | Obtain an attribute (must be present), simultaneously removing it from the header. |
| `opt_attr_del(name)` | Obtain an attribute (may be missing), simultaneously removing it from the header. |
| `has_attr(name)` | Returns `true` if the given attribute is present, otherwise returns `false`. Especially useful with the `filter` command; equivalent to the expression `opt_attr(name) != undefined`. |
### Examples
Count the number of sequences for each unique value of an 'abund' attribute in the FASTA headers (.e.g. `>id abund=3`), which could be the number of duplicates obtained by the *unique* command (see `st unique --help-vars`):
```sh
st count -k 'attr(abund)' seqs.fa
```
```
1	12019
2	2983
3	568
(...)
```
Summarize over a 'abund' attribute directly appended to the sequence ID like this `>id;abund=3`:
```sh
st count -k 'attr(abund)' --attr-fmt ';key=value' seqs.fa
```
Summarize over an attribute 'a', which may be 'undefined' (=missing) in some headers:
```sh
st count -k 'opt_attr(a)' seqs.fa
```
```
value1	6042
value2	1012
undefined	9566
```
## Access metadata from delimited text files
The following functions allow accessing associated metadata from plain delimited text files (optionally compressed, extension auto-recognized).
Metadata files must always contain a column with the sequence ID (default: 1st column; change with `--meta-idcol`).
The column delimiter is guessed from the extension or can be specified with `--meta-delim`. `.csv` is interpreted as comma(,)-delimited, `.tsv`/`.txt` or other (unknown) extensions are assumed to be tab-delimited.
The first line is implicitly assumed to contain column names if a non-numeric field name is requested, e.g. `meta(fieldname)`. Use `--meta-header` to explicitly enable header lines even if column names are all numeric.
Multiple metadata files can be supplied (`-m file1 -m file2 -m file3 ...`) and are addressed via `file-num` (see function descriptions). For maximum performance, provide metadata records in the same order as sequence records.
*Note:* Specify `--dup-ids` if the sequence input is expected to contain duplicate IDs (which is rather unusual). See the help page (`-h/--help`) for more information.
 

| | |
|-|-|
| `meta(column)`<br />`meta(column, file_number)` | Obtain a value an associated delimited text file supplied with `-m` or `--meta`. Individual columns from entries with matching record IDs are selected by number (1, 2, 3, etc.) or by their name according to the column names in the first row. Missing entries are not allowed. Column names can be in 'single' or "double" quotes (but quoting is only required in Javascript expressions).\<br /\>If there are multiple metadata files supplied with -m/--meta (`-m file1 -m file2 -m file3, ...`), the specific file can be referenced by supplying `\<file-number\>` (1, 2, 3, ...) as first argument, followed by the column number or name. This is not necessary if only a single file is supplied. |
| `opt_meta(column)`<br />`opt_meta(column, file_number)` | Like `meta(...)`, but metadata entries can be missing, i.e. not every sequence record ID needs a matching metadata entry. Missing values will result in 'undefined' if written to the output (= undefined in JavaScript expressions). |
| `has_meta`<br />`has_meta(file_number)` | Returns `true` if the given record has a metadata entry with the same ID in the in the given file. In case of multiple files, the file number must be supplied as an argument. |
### Examples
Add taxonomic lineages to the FASTA headers (after a space). The taxonomy is stored in a GZIP-compressed TSV file (column no. 2) to the FASTA headers:
```sh
st set -m taxonomy.tsv.gz -d '{meta(2)}' input.fa > output.fa
```
```
>id1 k__Fungi,p__Ascomycota,c__Sordariomycetes,(...),s__Trichoderma_atroviride
SEQUENCE
>id2 k__Fungi,p__Ascomycota,c__Eurotiomycetes,(...),s__Penicillium_aurantiocandidum
SEQUENCE
(...)
```
Add metadata from an Excel-generated CSV file (semicolon delimiter) to sequence headers as attributes (`-a/--attr`):
```sh
st pass -m metadata.csv --meta-sep ';' -a 'info={meta("column name")}' input.fa > output.fa
```
```
>id1 info=some_value
SEQUENCE
>id2 info=other_value
SEQUENCE
(...)
```
Extract subsequences given a set of coordinates stored in a BED file (equivalent to `bedtools getfasta`):
```sh
st trim -m coordinates.bed -0 {meta(2)}..{meta(3)} input.fa > output.fa
```
Filter sequences by ID, retaining only those present in the given text file:
```sh
st filter -m selected_ids.txt 'has_meta()' input.fa > output.fa
```
## Expressions (JavaScript)
Expressions with variables, from simple mathematical operations to arbitrarily complex JavaScript code.
Expressions are always enclosed in { curly brackets }. These brackets are optional for simple variables/functions in some cases, but mandatory for expressions. In addition, the 'filter' command takes an expression (without { brackets }).
 
Instead of JavaScript code, it is possible to refer to a source file using 'file:path.js'.
 
*Returned value*: For simple one-liner expressions, the value is directly used. More complex scripts with multiple statements (if/else, loops, etc.) explicitly require a `return` statement to return the value.
 

### Examples
Calculate the number of ambiguous bases in a set of DNA sequences and add the result as an attribute (ambig=...) to the header:
```sh
st pass -a ambig='{seqlen - charcount("ACGT")}' seqs.fasta
```
```
>id1 ambig=3
TCNTTAWTAACCTGATTAN
>id2 ambig=0
GGAGGATCCGAGCG
(...)
```
Discard sequences with >1% ambiguous bases or sequences shorter than 100bp:
```sh
st filter 'charcount("ACGT") / seqlen >= 0.99 && seqlen >= 100' seqs.fasta
```
Distribute sequences into different files by a slightly complicated condition. Note the 'return' statments are are necessary here, since this is not a simple expression. With even longer code, consider using an extra script and supplying -o "outdir/{file:code.js}.fasta" instead:
```sh
st split -po "outdir/{ if (id.startsWith('some_prefix_')) { return 'file_1' } return 'file_2' }.fasta" input.fasta
```
```
There should be two files now (`ls file_*.fasta`):
file_1.fasta
file_2.fasta
```
## Data conversion and transformation


| | |
|-|-|
| `num(expression)` | Converts any expression or value to a decimal number. Missing (undefined/null) values are left as-is. |
| `bin(expression)`<br />`bin(expression, interval)` | Groups a continuous numeric number into discrete bins with a given interval. The intervals are represented as '(start, end]', whereby start \<= value \< end; the intervals are thus open on the left as indicated by '(', and closed on the right, as indicated by ']'. If not interval is given, a default width of 1 is assumed. |
### Examples
Summarize by a numeric header attribute in the form '>id n=3':
```sh
st count -k 'num(attr("n"))' seqs.fa
```
```
1	1882
2	901
3	94
(...)
```
Summarize the distribution of the GC content in a set of DNA sequences in 5% intervals:
```sh
st count -k 'bin(gc_percent, 5)' seqs.fa
```
```
(15, 20]	73
(20, 25]	3443
(25, 30]	14138
(30, 35]	34829
(35, 40]	20354
(40, 45]	12142
(45, 50]	14019
(50, 55]	968
(55, 60]	8
```
