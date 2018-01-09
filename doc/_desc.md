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
to STDOUT or a file (`-o`, useful for [format conversion](wiki/pass)). Commands can
be easily chained using the pipe.

Use `seqtool <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](wiki/opts).


## Performance

The following run time comparison of diffferent tasks aims to give a quick overview but is not
comprehensive by any means. Comparisons to a selection of other tools/toolsets are shown if
there exists an equivalent operation. For all commands, a 1.1 Gb FASTQ file
containing 1.73 billion Illumina reads of 150-500 bp length was used. They were
run on a Mac Pro (Mid 2010, 2.8 GHz Quad-Core Intel Xeon, OS X 10.13) ([script](scripts/time.sh)).

|      | seqtool | [4 threads] | [seqtk](https://github.com/lh3/seqtk) | [seqkit](https://github.com/shenwei356/seqkit/) | [FASTX](https://github.com/agordon/fastx_toolkit) | [biopieces](http://maasha.github.io/biopieces/) |
|-----------------------------------------|---------|-------------|--------|--------|------------|-----------|
| Simple [counting](wiki/count)                | 0.41s  |             |        |        |            | 30.3s    |
| [Conversion](wiki/pass) to FASTA       | 0.80s  |             | 1.90s | 3.73s | 2min 32s | 1min 8s  |
| Reverse complement                      | 2.24s  | 0.79s      | 3.80s |  7.8s | 4min 25s | 1min 11s |
| [Random subsampling](wiki/sample) (to 10%)   | 0.69s  |             | 1.61s |  2.40s |            |           |
| [DNA to RNA (T -> U)](wiki/replace)          | 6.35s  | 2.05s      |        | 4.85s  | 4min 59s  | 1min 21s |
| [Remove short sequences](filter)      | 1.03s |      | 2.29s | 2.41s  |  | 1min 14s |
| [Summarize GC content](wiki/count)           | 3.60s  |             |        |        |            |           |
| .. with [math formula](wiki/variables#math-expressions) (GC% / 100)| 3.64s  |        |        |        |            |           |
| Summarize GC content stored in [attribute](wiki/attributes) | 0.97s  |    |           ||  |  |
| [Find 5' primer with max. 4 mismatches](wiki/find#algorithms-and-performance) | 52.1s  | 13.5s  |  |  |  |  |  |

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
