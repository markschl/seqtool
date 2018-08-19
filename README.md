**Seqtool** is a  fast and flexible command line program for dealing with
large amounts of biological sequences. It can read and write the
**FASTA**, **FASTQ** and **QUAL** files, as well as **CSV** and other delimited files. It also handles different common compression formats out of the box.
The tool is written in [Rust](https://www.rust-lang.org) and aims at solving
simple tasks that might otherwise only be solved by writing
custom scripts while being very fast. It uses
[seq_io](https://github.com/markschl/seq_io) and
[Rust-Bio](http://rust-bio.github.io/), amongst others,
and compiles to a standalone binary named `st`. [See below](#installing) for
instructions.


**Features:**

* [Format conversion](https://github.com/markschl/seqtool/wiki/pass), including different FASTQ variants.
  File extensions are auto-recognized if possible
* Many commands for summarizing, viewing, searching, shuffling
  and modifying sequences
* [Variables](https://github.com/markschl/seqtool/wiki/variables) allow to integrate sequence properties, metadata
  from [sequence headers](https://github.com/markschl/seqtool/wiki/attributes) and from [other files](https://github.com/markschl/seqtool/wiki/lists),
  and enable a flexible configuration of commands
* [Filtering](https://github.com/markschl/seqtool/wiki/filter) of sequences using mathematical expressions containing
  variables
* Passing metadata of FASTA/FASTQ sequences between commands is made easy by
  the ability to write and parse [sequence attributes](https://github.com/markschl/seqtool/wiki/attributes), which
  are key=value annotations in the sequence headers.
* Commands can be connected using the pipe (`|`) operator.



[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)
[![FOSSA Status](https://app.fossa.io/api/projects/git%2Bgithub.com%2Fmarkschl%2Fseqtool.svg?type=shield)](https://app.fossa.io/projects/git%2Bgithub.com%2Fmarkschl%2Fseqtool?ref=badge_shield)

## Commands
### Basic conversion / editing
* **[pass](https://github.com/markschl/seqtool/wiki/pass)**: This command is useful for converting from one format to another
and/or setting attributes.

### Information about sequences
* **[view](https://github.com/markschl/seqtool/wiki/view)**: View biological sequences, coloured by base / amino acid, or by sequence quality.
The output is automatically forwarded to the 'less' pager on UNIX.
* **[count](https://github.com/markschl/seqtool/wiki/count)**: This command counts the number of sequences and prints the number to STDOUT. Advanced
grouping of sequences is possible by supplying or more key strings containing
variables (-k).
* **[stat](https://github.com/markschl/seqtool/wiki/stat)**: Invalid arguments.

### Subsetting/shuffling sequences
* **[head](https://github.com/markschl/seqtool/wiki/head)**: Returns the first sequences of the input.
* **[tail](https://github.com/markschl/seqtool/wiki/tail)**: Returns the last sequences of the input.
* **[slice](https://github.com/markschl/seqtool/wiki/slice)**: Get a slice of the sequences within a defined range.
* **[sample](https://github.com/markschl/seqtool/wiki/sample)**: Return a random subset of sequences.
* **[filter](https://github.com/markschl/seqtool/wiki/filter)**: Filters sequences by a mathematical expression which may contain any variable.
* **[split](https://github.com/markschl/seqtool/wiki/split)**: This command distributes sequences into multiple files based on different
criteria. In contrast to other commands, the output (-o) argument can
contain variables in order to determine the file path for each sequence.
* **[interleave](https://github.com/markschl/seqtool/wiki/interleave)**: Interleaves records of all files in the input. The records will returned in
the same order as the files.

### Searching and replacing
* **[find](https://github.com/markschl/seqtool/wiki/find)**: Fast searching for one or more patterns in sequences or ids/descriptions,
with optional multithreading.
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
* **[concat](https://github.com/markschl/seqtool/wiki/concat)**: Concatenates sequences/alignments from different files in the order
in which they are provided. Fails if the IDs don't match.

## Installing

Binaries for Linux, Mac OS X and Windows can be
[downloaded from the releases section](https://github.com/markschl/seqtool/releases/latest).
For compiling from source, [install Rust](https://www.rust-lang.org), download the source
code; and inside the root directory type `cargo build --release`. The binary is found in
*target/release/*.


## Usage

```
st <command> [<options>] [<files>...]
```

All commands accept one or multiple files and STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](https://github.com/markschl/seqtool/wiki/pass)). Commands can
be easily chained using the pipe.

Use `st <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](https://github.com/markschl/seqtool/wiki/opts).


## Performance

The following run time comparison of diffferent tasks aims to give a quick overview but is not
comprehensive by any means. Comparisons to a selection of other tools/toolsets are shown if
there exists an equivalent operation. For all commands, a 1.1 Gb FASTQ file
containing 1.73 million Illumina reads of 150-500 bp length was used. They were
run on a Mac Pro (Mid 2010, 2.8 GHz Quad-Core Intel Xeon, OS X 10.9)
([script](https://github.com/markschl/seqtool/blob/master/scripts/time.sh)).

|      | seqtool | [4 threads] | [seqtk](https://github.com/lh3/seqtk) | [seqkit](https://github.com/shenwei356/seqkit/) | [FASTX](https://github.com/agordon/fastx_toolkit) | [biopieces](http://maasha.github.io/biopieces/) |
|-----------------------------------------|-------|-----------|--------|--------|------------|-----------|
| Simple [counting](https://github.com/markschl/seqtool/wiki/count)           | 0.62s |           |        |        |            | 46.99s    |
| [Conversion](https://github.com/markschl/seqtool/wiki/pass) to FASTA       | 1.20s  |           | 2.85s | 4.93s | 3min 38.4s | 3min 37.8s  |
| Reverse complement                      | 3.91s | 1.14s     | 5.46s |  10.14s | 6min 11.8s | 1m33.6s |
| [Random subsampling](https://github.com/markschl/seqtool/wiki/sample) (to 10%)   | 0.83s  |             | 2.05s |  2.54s |            |           |
| [DNA to RNA (T -> U)](https://github.com/markschl/seqtool/wiki/replace)          | 8.03s  | 2.35s|        | 6.13s  | 7min 9.4s  | 1min 49.1s |
| [Remove short sequences](https://github.com/markschl/seqtool/wiki/filter)      | 1.62s |      | 3.45s | 2.91s  |  | 1min 23.6s |
| [Summarize GC content](https://github.com/markschl/seqtool/wiki/count)           | 4.45s  |             |        |        |            |           |
| .. with [math formula](https://github.com/markschl/seqtool/wiki/variables#math-expressions) (GC% / 100)| 4.55s  |        |        |        |   |   |
| [Find forward primers with max. 4 mismatches](https://github.com/markschl/seqtool/wiki/find#algorithms-and-performance) | 8.02s | 2.34s  |  |  |  |  |  |
| [Remove the primers if found \(1.36 M seqs\)](https://github.com/markschl/seqtool/wiki/trim#using-variables) | 2.26s |   |  |  |  |  |  |

Simple counting is the fastest operation, faster than the UNIX line counting
command (`wc -l`, 2.70s) on OS X. The commands `find`, `replace` and `revcomp`
additionally profit from multithreading.

Compressed files are recognized based on their extension (Example:
`st . seqs.lz4`). Compressed I/O is done in a separate thread by default,
which makes reading/writing faster than via the pipe (e.g. `lz4 -dc seqs.lz4 | st . `),
with the exception of GZIP on OS X. Reading/writing [LZ4](http://lz4.github.io/lz4)
is almost as fast as reading uncompressed input. Writing LZ4 is only slightly
slower while providing a reasonable compression ratio. For files stored on
slow hard disks, LZ4 can be even faster than uncompressed I/O.
[Zstandard](http://facebook.github.io/zstd) was added because it provides a
better compression than LZ4 while still being very fast.


| format       | file size (Mb) | read   | (piped) | compress   | (piped)    |
|--------------|----------------|--------|---------|------------|------------|
| uncompressed<sup>1</sup>| 1199| 1.28s  | -       | 1.23s      | -          |
| LZ4          | 192            | 1.36s  | 2.71s   | 2.60s      | 3.95s      |
| GZIP         | 101            | 10.98s | 6.15s   | 53.83s     | 50.63s     |
| Zstandard    | 86             | 2.33s  | 3.62s   | 4.29s      | 5.79s      |
| BZIP2        | 60             | 32.85s | 30.25s  | 3min 35.3s | 4min 20.4s |

<sup>1</sup> Using `-T/--read-thread` / `--write-thread`


## Further improvements

I am grateful for comments and ideas on how to improve the tool and also about
feedback in general. Commands for sorting, dereplication and for working with
alignments are partly implemented but not ready.

Since the tool is quite new, it is possible that there are bugs, even if
[tests for every command](https://github.com/markschl/seqtool/tree/master/src/test)
and for most parameter combinations have been written.
