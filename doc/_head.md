**Seqtool** is a  general purpose command line program for dealing with large
amounts of biological sequences, transforming and filtering them.
It can read and write **FASTA**, **FASTQ** and **CSV** files
supports different compression algorithms (**GZIP**, **BZIP2**, **LZ4**).
It uses the [Rust-Bio](http://rust-bio.github.io/) and [seq_io](https://github.com/markschl/seq_io)
libraries, amongst others, and compiles to a standalone binary.

The tool evolved from a simple python script and was rewritten in the *Rust*
language. It and aims at solving simple tasks that might otherwise only be solved
by writing custom scripts. This is possible with the use
of [variables](wiki/variables) and mathematical expressions.
In contrast to frameworks like [biopieces](https://github.com/maasha/biopieces),
no custom format is used for passing information between commands. Instead the tool uses '[properties](wiki/properties)', which are key=value strings added to the sequence headers, or custom CSV fields.


[![UNIX build status](https://travis-ci.org/markschl/seqtool.svg?branch=master)](https://travis-ci.org/markschl/seqtool/)
[![Windows build status](https://ci.appveyor.com/api/projects/status/github/markschl/seqtool?svg=true)](https://ci.appveyor.com/project/markschl/seqtool)
