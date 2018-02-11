**Seqtool** is a  fast and flexible command line program for dealing with
large amounts of biological sequences. It can read and write
**FASTA**, **FASTQ** and **CSV** files and handles different common
compression formats (GZIP, BZIP2), but also supports newer/faster compression
algorithms ([LZ4](http://lz4.github.io/lz4) and
[Zstandard](http://facebook.github.io/zstd)) out of the box.


The tool is written in [Rust](https://www.rust-lang.org) and aims at solving
simple tasks that might otherwise only be solved by writing
custom scripts while being very fast. This is possible with the use of
[variables and mathematical expressions](wiki/variables).
In contrast to [biopieces](https://github.com/maasha/biopieces),
no custom format is used for passing information between commands.
Instead, it is possible to use '[attributes](wiki/attributes)', which are
key=value strings added to the sequence headers, or custom CSV fields.

It uses the [Rust-Bio](http://rust-bio.github.io/) and
[seq_io](https://github.com/markschl/seq_io) libraries, amongst others
and compiles to a standalone binary.


[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)
