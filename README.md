**Seqtool** is a  general purpose command line program for dealing with
large amounts of biological sequences. It can read and write 
**FASTA**, **FASTQ** and **CSV** files and supports different compression
algorithms (**GZIP**, **BZIP2**, **LZ4**), auto-recognizing the 
extensions.

The tool is written in [Rust](https://www.rust-lang.org) and aims at solving simple tasks that might otherwise only be solved by writing
custom scripts. This is possible with the use of 
[variables and mathematical expressions](https://github.com/markschl/seqtool/wiki/variables).
In contrast to [biopieces](https://github.com/maasha/biopieces),
no custom format is used for passing information between commands.
Instead, it is possible to use '[attributes](https://github.com/markschl/seqtool/wiki/attributes)', which are key=value strings added to the sequence headers, or custom CSV fields.

It uses the [Rust-Bio](http://rust-bio.github.io/) and 
[seq_io](https://github.com/markschl/seq_io) libraries, amongst others
and compiles to a standalone binary.


[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)

## Commands
### Basic conversion / editing
* **[pass](https://github.com/markschl/seqtool/wiki/pass)**: This command is useful for converting from one format to another
and/or setting attributes.

### Information about sequences
* **[count](https://github.com/markschl/seqtool/wiki/count)**: This command counts the number of sequences and prints the number to STDOUT. Advanced 
grouping of sequences is possible by supplying or more key strings containing
variables (-k).
* **[stat](https://github.com/markschl/seqtool/wiki/stat)**: Returns per sequence statistics as tab delimited list. All statistical variables
(s:<variable>) can be used.

### Subsetting/shuffling sequences
* **[head](https://github.com/markschl/seqtool/wiki/head)**: Returns the first sequences of the input.
* **[tail](https://github.com/markschl/seqtool/wiki/tail)**: Returns the last sequences of the input.
* **[slice](https://github.com/markschl/seqtool/wiki/slice)**: Get a slice of the sequences within a defined range.
* **[sample](https://github.com/markschl/seqtool/wiki/sample)**: Return a random subset of sequences.
* **[filter](https://github.com/markschl/seqtool/wiki/filter)**: Filters sequences by a mathematical expression which may contain any variable.
* **[split](https://github.com/markschl/seqtool/wiki/split)**: This command distributes sequences into multiple files based on different
criteria.

### Searching and replacing
* **[find](https://github.com/markschl/seqtool/wiki/find)**: Searches for one or more patterns in sequences or ids / descriptions,
optional multithreading.
* **[replace](https://github.com/markschl/seqtool/wiki/replace)**: This command does fast search and replace for patterns in sequences
or ids/descriptions.

### Modifying commands
* **[del](https://github.com/markschl/seqtool/wiki/del)**: Deletes description field or attributes.
* **[set](https://github.com/markschl/seqtool/wiki/set)**: Replaces the contents of sequence IDs, descriptions or sequences.
* **[trim](https://github.com/markschl/seqtool/wiki/trim)**: Trims sequences to a given range.
* **[mask](https://github.com/markschl/seqtool/wiki/mask)**: Masks the sequence within a given range or comma delimited list of ranges
by converting to lowercase (soft mask) or replacing with a character (hard
masking). Reverting soft masking is also possible.
* **[upper](https://github.com/markschl/seqtool/wiki/upper)**: Converts all characters in the sequence to uppercase.
* **[lower](https://github.com/markschl/seqtool/wiki/lower)**: Converts all characters in the sequence to lowercase.
* **[revcomp](https://github.com/markschl/seqtool/wiki/revcomp)**: Reverse complements DNA sequences. If quality scores are present,
their order is just reversed.
## Installing

Binaries for Linux, Mac OS X and Windows can be
[downloaded from the releases section](https://github.com/markschl/seqtool/releases/latest).
For compiling from source, [install Rust](https://www.rust-lang.org), download the source
code; and inside the root directory type `cargo build --release`. The binary is found in
*target/release/*.


## Usage

```
seqtool <command> [<options>] [<files>...]
```

All commands accept one or multiple files, or STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](https://github.com/markschl/seqtool/wiki/pass)). Commands can
be easily chained using the pipe.

Use `seqtool <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](https://github.com/markschl/seqtool/wiki/opts).


## Performance

The following run time comparison of diffferent tasks aims to give a quick overview but is not
comprehensive by any means. Comparisons to a selection of other tools/toolsets are shown if
there exists an equivalent operation. For all commands, a 1.1 Gb FASTQ file
containing 1.73 billion Illumina reads of 150-500 bp length was used. They were
run on a Mac Pro (Mid 2010, 2.8 GHz Quad-Core Intel Xeon, OS X 10.13) ([script](scripts/time.sh)).

|      | seqtool | [4 threads] | [seqtk](https://github.com/lh3/seqtk) | [seqkit](https://github.com/shenwei356/seqkit/) | [FASTX](https://github.com/agordon/fastx_toolkit) | [biopieces](http://maasha.github.io/biopieces/) |
|-----------------------------------------|---------|-------------|--------|--------|------------|-----------|
| Simple [counting](https://github.com/markschl/seqtool/wiki/count)                | 0.41s  |             |        |        |            | 30.3s    |
| [Conversion](https://github.com/markschl/seqtool/wiki/pass) to FASTA       | 0.80s  |             | 1.90s | 3.73s | 2min 32s | 1min 8s  |
| Reverse complement                      | 2.24s  | 0.79s      | 3.80s |  7.8s | 4min 25s | 1min 11s |
| [Random subsampling](https://github.com/markschl/seqtool/wiki/sample) (to 10%)   | 0.69s  |             | 1.61s |  2.40s |            |           |
| [DNA to RNA (T -> U)](https://github.com/markschl/seqtool/wiki/replace)          | 6.35s  | 2.05s      |        | 4.85s  | 4min 59s  | 1min 21s |
| [Remove short sequences](filter)      | 1.03s |      | 2.29s | 2.41s  |  | 1min 14s |
| [Summarize GC content](https://github.com/markschl/seqtool/wiki/count)           | 3.60s  |             |        |        |            |           |
| .. with [math formula](https://github.com/markschl/seqtool/wiki/variables#math-expressions) (GC% / 100)| 3.64s  |        |        |        |            |           |
| Summarize GC content stored in [attribute](https://github.com/markschl/seqtool/wiki/attributes) | 0.97s  |    |           ||  |  |
| [Find 5' primer with max. 4 mismatches](https://github.com/markschl/seqtool/wiki/find#algorithms-and-performance) | 52.1s  | 13.5s  |  |  |  |  |  |

Simple counting is the fastest operation, faster than the UNIX line counting
command (`wc -l`, 2.70s) on OS X. The commands `find`, `replace` and `revcomp`
additionally profit from multithreading.

Compressed I/O is done in a separate thread by default. For LZ4,
this is faster than getting the input via the pipe
(`seqtool . seqs.lz4` vs. `lz4 -dc seqs.lz4 | seqtool . `). This seems not to be
true for GZIP, currently. Reading LZ4 is almost as fast as reading
uncompressed input. Writing LZ4 is only slightly slower while providing
a reasonable compression ratio. For files stored on slow hard disks,
LZ4 can be even faster than uncompressed I/O.


| format                 |              | seqtool | seqtool piped |
|------------------------|--------------|---------|---------------|
| uncompressed (1168 Mb) | read + write | 0.88 s  | -             |
| LZ4 (234 Mb)           | decompress   | 1.16 s  | 2.15 s        |
|                        | compress     | 2.54 s  | 3.65 s        |
| GZIP (130 Mb)          | decompress   | 10.5 s  | 3.72 s        |
|                        | compress     | 52.3 s  | 45.8 s        |


## Further improvements

I am grateful for comments and ideas on how to improve the tool and also about
feedback in general. Commands for sorting, dereplication and for working with
alignments are partly implemented but not ready. I would also like to add a
flexible filtering command based on math expressions, however I'm not yet sure
on which library this should be based.

Since the tool is quite new, it is possible that there are bugs, even if
[tests for every command](https://github.com/markschl/seqtool/tree/master/src/test)
have been written (although not for every parameter combination).
