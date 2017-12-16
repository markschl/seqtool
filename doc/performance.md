# Performance comparisons

Runtime comparison of different tasks done with a 1.1 Gb FASTQ file
containing 1.73 billion Illumina reads of 150-500 bp length. The commands were
run on a Mac Pro (Mid 2010, 2.8 GHz Quad-Core Intel Xeon, OS X 10.13).

Simple counting is the fastest operation. It even beats the `wc -l` command in this
case (although not always with longer sequences). Writing FASTA is slower, but still
very fast. Commands like `find` and `replace` additionally profit from multithreading.

| _    | seqtool | (4 threads) | [seqtk](https://github.com/lh3/seqtk)  | [FASTX](https://github.com/agordon/fastx_toolkit) | [biopieces](http://maasha.github.io/biopieces/) | wc -l  |
|-----------------------------------------|---------|-------------|--------|------------|-----------|--------|
| Simple [counting](count)                | 0.41 s  |             |        |            | 30.3 s    | 2.70 s |
| [Conversion](conversion) to FASTA       | 0.80 s  |             | 1.90 s | 2 min 32 s | 1 min 8s  |        |
| Reverse complement                      | 2.24 s  | 0.79 s      | 3.80 s | 4 min 25 s | 1 min 11s |        |
| [Random subsampling](sample) (`-f 0.1`) | 0.69 s  |             | 1.61 s |            |           |        |
| [RNA to DNA (T -> U)](replace)          | 6.35 s  | 2.05 s      |        | 4min 59 s  | 1 min 23s |        |
| [Summarize GC content](count)           | 3.60 s  |             |        |            |           |        |
| .. with math formula (GC% + 0)          | 3.84 s  |             |        |            |           |        |
| Summarize GC content stored in [property](properties) | 0.97 s  |    |     |  |  | |
| [Find 5' primer with max. 4 mismatches](find#algorithms-and-performance) | 52.1 s  | 13.5 s  |  |  |  |  | |

The tool can read and write the compression formats GZIP, BZIP2 and LZ4.
Reading LZ4 is as fast as reading uncompressed FASTQ. Writing LZ4 is
only slightly slower while providing a reasonable compression ratio.
For files stored on slow hard disks however, it can be even faster.
Compressed I/O is done in a separate thread by default. For LZ4,
this is faster than getting the input via the pipe
(`seqtool . seqs.lz4` vs. `lz4 -dc seqs.lz4 | seqtool . `). This seems not to be
true for GZIP, currently.

| format                 |              | seqtool | seqtool piped |
|------------------------|--------------|---------|---------------|
| uncompressed (1168 Mb) | read + write | 0.88 s  | -             |
| LZ4 (234 Mb)           | decompress   | 1.16 s  | 2.15 s        |
|                        | compress     | 2.54 s  | 3.65 s        |
| GZIP (130 Mb)          | decompress   | 10.5 s  | 3.72 s        |
|                        | compress     | 52.3 s  | 45.8 s        |
