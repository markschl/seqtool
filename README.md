**Seqtool** is a  general purpose command line program for dealing with large
amounts of biological sequences, transforming and filtering them.
It can read and write **FASTA**, **FASTQ** and **CSV** files
supports different compression algorithms (**GZIP**, **BZIP2**, **LZ4**). It uses the [Rust-Bio](http://rust-bio.github.io/) and [seq_io](https://github.com/markschl/seq_io)
libraries, amongst others, and compiles to a standalone binary.

The tool evolved from a simple python script and was rewritten in the *Rust*
language. It and aims at solving simple tasks that might otherwise only be solved
by writing custom scripts. This is possible with the use
of [variables](variables). Commands can be connected with the pipe operator.
In contrast to frameworks like [biopieces](https://github.com/maasha/biopieces),
no custom format is used for passing information between commands, but data can
be stored in [properties](properties) in the form 'key=value' added to the sequence
headers, or as CSV fields.
# Commands

### Basic conversion / editing
* **[pass](pass)**: This command is useful for converting from one format to another
and/or setting properties.

### Information about sequences
* **[count](count)**: This command counts the number of sequences and prints the number to STDOUT. Optionally,
Advanced grouping of sequences is possible by supplying or more key strings containing
variables (-k).
* **[stat](stat)**: Returns per sequence statistics as tab delimited list. All statistical variables
(s:<variable>) can be used.

### Subsetting/shuffling sequences
* **[head](head)**: Returns the first sequences of the input.
* **[tail](tail)**: Returns the last sequences of the input.
* **[slice](slice)**: Get a slice of the sequences within a defined range.
* **[sample](sample)**: Return a random subset of sequences.
* **[split](split)**: This command distributes sequences into multiple files based on different
criteria.

### Searching and replacing
* **[find](find)**: Searches for one or more patterns in sequences or ids / descriptions,
optional multithreading.
* **[replace](replace)**: This command searches for patterns in sequences or ids/descriptions
and replaces them by <replacement>. Approximative searches
are not possible, use 'match' for this.

### Modifying commands
* **[del](del)**: Deletes description field or properties.
* **[set](set)**: Replaces the contents of sequence IDs, descriptions or sequences.
* **[trim](trim)**: Trims sequences to a given range.
* **[mask](mask)**: Masks the sequence within a given range or comma delimited list of ranges
by converting to lowercase (soft mask) or replacing with a character (hard
masking). Reverting soft masking is also possible.
* **[upper](upper)**: Converts all characters in the sequence to uppercase.
* **[lower](lower)**: Converts all characters in the sequence to lowercase.
* **[revcomp](revcomp)**: Reverse complements DNA sequences. If quality scores are present,
their order is just reversed.

## Usage

```
seqtool <command> [<options>] [<files>...]
```

All commands accept one or multiple files, or STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](wiki/pass)). Commands can
be easily chained using the pipe.

### Options recognized by all commands

Use `seqtool -h` or [see here](wiki/opts) for a full list of options.

### Performance

Seqtool is very fast for most tasks, see [here for a comparison with other tools](wiki/performance).
