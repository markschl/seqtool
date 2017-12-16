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
