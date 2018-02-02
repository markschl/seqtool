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

All commands accept one or multiple files and STDIN input. The output is written
to STDOUT or a file (`-o`, useful for [format conversion](wiki/pass)). Commands can
be easily chained using the pipe.

Use `seqtool <command> -h` to see all available options. A full list of options
that are accepted by all commands can be [found here](wiki/opts).


## Performance

The following run time comparison of diffferent tasks aims to give a quick overview but is not
comprehensive by any means. Comparisons to a selection of other tools/toolsets are shown if
there exists an equivalent operation. For all commands, a 1.1 Gb FASTQ file
containing 1.73 billion Illumina reads of 150-500 bp length was used. They were
run on a Mac Pro (Mid 2010, 2.8 GHz Quad-Core Intel Xeon, OS X 10.9)
([script](https://github.com/markschl/seqtool/blob/master/scripts/time.sh)).

|      | seqtool | [4 threads] | [seqtk](https://github.com/lh3/seqtk) | [seqkit](https://github.com/shenwei356/seqkit/) | [FASTX](https://github.com/agordon/fastx_toolkit) | [biopieces](http://maasha.github.io/biopieces/) |
|-----------------------------------------|-------|-----------|--------|--------|------------|-----------|
| Simple [counting](wiki/count)           | 0.62s |           |        |        |            | 46.99s    |
| [Conversion](wiki/pass) to FASTA       | 1.20s  |           | 2.85s | 4.93s | 3min 38.4s | 3min 37.8s  |
| Reverse complement                      | 3.91s | 1.14s     | 5.46s |  10.14s | 6min 11.8s | 1m33.6s |
| [Random subsampling](wiki/sample) (to 10%)   | 0.83s  |             | 2.05s |  2.54s |            |           |
| [DNA to RNA (T -> U)](wiki/replace)          | 8.03s  | 2.35s|        | 6.13s  | 7min 9.4s  | 1min 49.1s |
| [Remove short sequences](wiki/filter)      | 1.62s |      | 3.45s | 2.91s  |  | 1min 23.6s |
| [Summarize GC content](wiki/count)           | 4.45s  |             |        |        |            |           |
| .. with [math formula](wiki/variables#math-expressions) (GC% / 100)| 4.55s  |        |        |        |   |   |
| Summarize GC content stored in [attribute](wiki/attributes) | 1.55s  |    |           ||  |  |
| [Find 5' primer with max. 4 mismatches](wiki/find#algorithms-and-performance) | 52.1s  | 13.5s  |  |  |  |  |  |

Simple counting is the fastest operation, faster than the UNIX line counting
command (`wc -l`, 2.70s) on OS X. The commands `find`, `replace` and `revcomp`
additionally profit from multithreading.

Compressed files are recognized based on their extension (Example:
`seqtool . seqs.lz4`). Compressed I/O is done in a separate thread by default,
which makes reading/writing faster than via the pipe (e.g. `lz4 -dc seqs.lz4 | seqtool . `),
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
