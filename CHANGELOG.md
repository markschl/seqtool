## v0.2.4

**Binary renamed:** The binary was renamed from `seqtool` to `st`, because
typing the rather long name repeatedly can be tiring (-: The documentation
was updated accordingly

**New commands:**
* *view* command for viewing sequences in terminal with colored background

**Changes / additions:**
* Seqtool now handles quality scores associated with sequences. This includes:
  - Support for converting between FASTQ format variants
  - The `exp_err` statistics variable, which represents the total number of expected
    errors in a sequence, according to the quality scores. This allows filtering
    by sequence quality, like `fastq_filter` from USEARCH and VSEARCH
  - 454-style QUAL files can be read an written

* `--format` renamed to `--fmt`, and `--outformat` renamed to `--to`

**Bug fixes:**
- `seq_io` was updated, containing two small bug fixes. Both bugs were very
  unlikely to be hit and of low severity

## v0.2.3

**New commands:**
* *concat* command
* *interleave* command

**Changes / additions:**
* The *find* command is now much faster for approximate searching.
   The starting position (f:start variable) can be different in some
   cases where several alignments with the same distance and end position are
   possible.
  **Note:** Pattern length for approximate searching is now limited to 64.

* Added a shortcut `-T` for `--read-thread`. Adding this option
  (and eventually `--write-thread`) can speed up many commands.
  Since there are no comprehensive benchmarks, the default
  is to read/write in the main thread.
* New *dirname* variable

**Bug fixes:**
* Compressed files produced by the *split* command were still truncated
  in some cases (related to bug fixed in v0.2.2). This is fixed now.
* Pattern type recognition in *find* command was not always correct

## v0.2.2

Important bugfixes:
* Writing compressed files was partly broken (truncated files) and was fixed
  now, along with many unit tests
* With `--fields`/`--txt`/`--csv`, column indices are now correctly interpreted
  as 1-based, not 0-based
* Fixed handling of NaN values in 'count' command
* `tail` returned one sequence too much.

Changes to behaviour:
* `-n` option removed from *slice* command, use a range (`..n`) instead.

New features:
* Added ZSTD (de-)compression which is almost as fast as LZ4, but has a better
  compression ratio
* Now allows for changing the compression level (`--compr-level`)
* Writing attributes to sequences removed by filtering (`--dropped` option of
  `find` and `filter`) is now possible.
* Other small improvements and better testing, almost all options have tests now.


## v0.2.1

* recognize '.tsv' and uppercase extensions
* report names of missing files
* small performance improvements


## v0.2.0

* added more advanced math expressions (ExprTk library)
* filter command
* renamed 'properties' to 'attributes'
* bug fixes, cleanups and more tests


v0.1.1: Initial release
