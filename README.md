**Seqtool** is a  general purpose command line program for dealing with large
amounts of biological sequences, transforming and filtering them.
It can read and write **FASTA**, **FASTQ** and **CSV** files
supports different compression algorithms (**GZIP**, **BZIP2**, **LZ4**).
It uses the [Rust-Bio](http://rust-bio.github.io/) and [seq_io](https://github.com/markschl/seq_io)
libraries, amongst others, and compiles to a standalone binary.

The tool evolved from a simple python script and was rewritten in the *Rust*
language. It and aims at solving simple tasks that might otherwise only be solved
by writing custom scripts. This is possible with the use
of [variables](https://github.com/markschl/seqtool/wiki/variables). Commands can be connected with the pipe operator.
In contrast to frameworks like [biopieces](https://github.com/maasha/biopieces),
no custom format is used for passing information between commands, but data can
be stored in [properties](https://github.com/markschl/seqtool/wiki/properties) in the form 'key=value' added to the sequence
headers, or as CSV fields.

[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)

# Commands
### Basic conversion / editing
* **[pass](https://github.com/markschl/seqtool/wiki/pass)**: This command is useful for converting from one format to another
and/or setting properties.

### Information about sequences
* **[count](https://github.com/markschl/seqtool/wiki/count)**: This command counts the number of sequences and prints the number to STDOUT. Optionally,
Advanced grouping of sequences is possible by supplying or more key strings containing
variables (-k).
* **[stat](https://github.com/markschl/seqtool/wiki/stat)**: Returns per sequence statistics as tab delimited list. All statistical variables
(s:<variable>) can be used.

### Subsetting/shuffling sequences
* **[head](https://github.com/markschl/seqtool/wiki/head)**: Returns the first sequences of the input.
* **[tail](https://github.com/markschl/seqtool/wiki/tail)**: Returns the last sequences of the input.
* **[slice](https://github.com/markschl/seqtool/wiki/slice)**: Get a slice of the sequences within a defined range.
* **[sample](https://github.com/markschl/seqtool/wiki/sample)**: Return a random subset of sequences.
* **[split](https://github.com/markschl/seqtool/wiki/split)**: This command distributes sequences into multiple files based on different
criteria.

### Searching and replacing
* **[find](https://github.com/markschl/seqtool/wiki/find)**: Searches for one or more patterns in sequences or ids / descriptions,
optional multithreading.
* **[replace](https://github.com/markschl/seqtool/wiki/replace)**: This command searches for patterns in sequences or ids/descriptions
and replaces them by <replacement>. Approximative searches
are not possible, use 'match' for this.

### Modifying commands
* **[del](https://github.com/markschl/seqtool/wiki/del)**: Deletes description field or properties.
* **[set](https://github.com/markschl/seqtool/wiki/set)**: Replaces the contents of sequence IDs, descriptions or sequences.
* **[trim](https://github.com/markschl/seqtool/wiki/trim)**: Trims sequences to a given range.
* **[mask](https://github.com/markschl/seqtool/wiki/mask)**: Masks the sequence within a given range or comma delimited list of ranges
by converting to lowercase (soft mask) or replacing with a character (hard
masking). Reverting soft masking is also possible.
* **[upper](https://github.com/markschl/seqtool/wiki/upper)**: Converts all characters in the sequence to uppercase.
* **[lower](https://github.com/markschl/seqtool/wiki/lower)**: Converts all characters in the sequence to lowercase.
* **[revcomp](https://github.com/markschl/seqtool/wiki/revcomp)**: Reverse complements DNA sequences. If quality scores are present,
their order is just reversed.

## Usage

```
seqtool <command> [<options>] [<files>...]
```

All commands accept one or multiple files, or STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](https://github.com/markschl/seqtool/wiki/pass)). Commands can
be easily chained using the pipe.

Use `seqtool <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](https://github.com/markschl/seqtool/wiki/opts).

## Installing

Binaries for Linux, Mac OS X and Windows can be
[downloaded from the releases section](https://github.com/markschl/seqtool/releases/latest).

## Performance

Seqtool is very fast for most tasks, see [here for a comparison with other tools](https://github.com/markschl/seqtool/wiki/performance).
