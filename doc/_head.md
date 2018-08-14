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

* [Format conversion](wiki/pass), including different FASTQ variants.
  File extensions are auto-recognized if possible
* Many commands for summarizing, viewing, searching, shuffling
  and modifying sequences
* [Variables](wiki/variables) are accepted by many commands, allowing
  to integrate sequence statistics, metadata from
  [sequence headers](wiki/attributes) and from [other files](wiki/lists).
* Flexible [filtering](wiki/filter) using mathematical expressions which
  can include any variable
* Many commands can be connected using the pipe (`|`) operator.


[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)
