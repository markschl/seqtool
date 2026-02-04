## v0.4.0 (2026-02-04)

* Improve `-v/--verbose` messages and add `--report` option (very simple JSON reports)
* Rename `seqhash_both` to `seqhash_min` (for checking equality irrespective of sequence strand)
* Avoid truncated output with *cmp* command (bugfix)
* Added script for comparing the pattern search with other tools (Cutadapt, USEARCH)
  in order to further validate the *find* command

## v0.4.0-beta.4 (2025-10-06)

* *Seqtool* is now dual-licensed under MIT OR Apache-2.0 ([9b0ebf0](https://github.com/markschl/seqtool/commit/9b0ebf0b9fc49f7d5178446989626672fc3be44b))
* Added a *cmp* command for comparing sequence sets
  ([5cf0b06](https://github.com/markschl/seqtool/commit/5cf0b06e9a13c26e33dded6d67c71966fd0c8ce1))
* Improved *view* command: implemented an internal pager with more features, which also works on
  Windows ([a1d6934](https://github.com/markschl/seqtool/commit/a1d69348e14cb1e8768e278797f559587783a0be))
* Improvements to the *find* command
  - Reviewed/adjusted fuzzy pattern matching. 
    The prioritization of hits with identical edit distance was improved and match positions are now
    comparable to the ones from Cutadapt
    ([3821e60](https://github.com/markschl/seqtool/commit/3821e60bd19984a1491801ee8ac525b00541dd3f),
    [a8eeea9](https://github.com/markschl/seqtool/commit/a8eeea9babee197ecb7d53bb9d2f3a9455b70540),
    [de34546](https://github.com/markschl/seqtool/commit/de345469629790569cc26bb7e4bfce211bdb1e57))
  - Case-insensitive search ([3f35c32](https://github.com/markschl/seqtool/commit/3f35c3209a205c3ea029452ce01c293609766d16))
  - Added `match_neg_range` variable/function ([824a027](https://github.com/markschl/seqtool/commit/824a02747bad288521140ec81c86c56070a58d38)),
   and hits can be referred to with negative indices (viewed from the last hit) in all functions
   ([8929f91](https://github.com/markschl/seqtool/commit/8929f9141738d9e283bcd743a89ad546c5178b73))
  - Slight behaviour change in case of pattern anchoring leading to faster searches
   ([5da8427](https://github.com/markschl/seqtool/commit/5da8427ff2443e952addccb4f6c57ff4e11e4f89))
* Small bugfixes ([99bf13e](https://github.com/markschl/seqtool/commit/99bf13edba77f1cf2e40b1baf866047b6c024795),
  [f4b1966](https://github.com/markschl/seqtool/commit/f4b1966a7e23f2738ddc564cb72b02db7966b117))
* Numerous smaller improvements & dependency updates; some dependencies were dropped

## v0.4.0-beta.3 (2025-05-01)

Besides dependency updates, the release includes:

* The ability to append records to the output file(s) instead of overwriting
  (`--append`) (7fe1893). This is especially useful for the `split` command.
* A new `--counts` option in the `split` command, which returns the record counts
  for each file (565b42c). This saves an extra `st count` command.
* Added some error messages to prevent panics (d91b1fc, 8156c45)

## v0.4.0-beta.2 (2024-07-08)

This release comes with substantial improvements and lots of internal code improvements.

First of all, we report the bugs :/

### Important bugfixes 🐞

Aside from minor bugs (listed further down), a few more important bugs were discovered
in *seqtool* v0.3.0, and are now fixed in v.0.4.0.

* Some multi-member GZIP and BZIP2 files were not read completely (#bc27f91).
  *seqtool* v0.3.0 stops parsing early, leading to truncated input.
  For example, `st count seqs.fastq.gz` may report a smaller sequence count, while
  `zcat seqs.fastq.gz | st count` returns the correct total count.
* Searching (*find*) and replacing (*replace*) in **multi-line** (wrapped) FASTA sequences
  can lead to incorrect results
  - *find*: `-f/--filter` wrongly returns one or several sequences *after* a sequence match,
    even if they don't contain the searched pattern. Match coordinates (`f:start`, `f:range`, etc.)
    are also wrong (shifted).
  - *replace*: some sequence lines can be duplicated in the output, leading to "inflated"
    sequences, which in the worst case can lead to huge output files.
  The bug was implicitly fixed while refactoring (#03ca54b).
* In the *find* command, `--max-shift-r/l` (now `--max-shift-start/end`) did not
  play together with `-f/--filter` or `-e/--exclude`, records were always returned
  if *any* match was found, irrespective of their position in the sequence.

### New function-like syntax for variables

The rather odd variable syntax (names containing `:` and `.`) was replaced by
a more familiar function-style syntax. The most notable changes are:

* Sequence attributes are accessed using `attr(name)` instead of `attr:name`.
  In some cases (JS expressions, see below), `name` will need to be quoted:
  `attr('name')`
* Associated metadata are now accessed using `meta(col_name)` or `meta(col_number)`
  instead of `l:col_name` or `l:col_number`.

It would be too laborious to list all syntax changes, so please refer to `st <command> --help-vars`
for a complete list of all available variables.

### JavaScript expressions

The ExprTk expression engine has been replaced with [QuickJS](https://bellard.org/quickjs),
which is a tiny and reasonably fast JavaScript engine directly embedded in *seqtool*
(thanks to the [rquickjs](https://github.com/DelSkayn/rquickjs) crate).
From simple math to more complicated expressions or even longer scripts evaluated on-the-fly
for every new sequence record - everything is handled by the *QuickJS* engine.
The use of JS is also advantageous, since it is a well-known language, which is
much more powerful than the previous *ExprTk* mini-language.
For simple math expressions, *QuickJS* is slower than *ExprTk*,
 but on the other hand the new engine is much more powerful and 
can also return strings (very useful in the `split` command). I'm still investigating
ways to speed up simple math operations, e.g. with a separate small math engine.

Second, JS expressions don't need the `{{ expression }}` syntax anymore, `{ expression }`
is enough. Seqtool will automatically check whether it is a JS expression or a simple
variable/function that doesn't need the JS engine.
The new function-like syntax for *seqtool*'s internal variables integrates quite nicely
with the JavaScript function syntax, even though there are still minor inconsistencies.

Most importantly, JavaScript would interpret *name* in `attr(name)` as a variable name,
while *seqtool* interprets it as an un-quoted string `"name"` (which is the name of the header attribute).
This may confuse users and we may add a feature switch to always enforce argument quoting,
but for now we have an informative error message that is displayed in those cases.

*Regex matching in JS*: the literal `/regex/` syntax cannot be used, use `new RegExp()` instead.

### `sort` and `unique` commands

These two new commands allow sorting and de-replicating by any variable/function/expression.
Both have a built-in (but configurable) memory limit, above which the tool will switch to
using sorted temporary files. This takes longer, but allows sorting or de-replicating
huge collections with limited memory.

For efficient de-replication, the *seqhash* and *seqhash_both* variables/functions
were introduced.

### New range notation

The Rust-like `start..end` has been replaced with the shorter Python-like
`start:end` syntax. Using dashes (`start-end`) is not an option because
like Python, *seqtool* supports negative coordinates (offset from end).

### Improvements to other commands

#### Pattern matching (`find`)

* *Better approximate/fuzzy matching:*: Fuzzy matching up to a given edit distance
  sometimes finds multiple possible alignments with the same starting position and edit distance.
  Instead of reporting the first possible hit, a custom gap penalty (`--gap-penalty`) allows
  selecting the optimal hit without too many InDels. This is especially useful when
  trimming primers where substitutions are usually more frequent than InDels.
  Fuzzy matching still relies on the edit distance as primary measure to select
  the best hit; enforcing ungapped alignments is currently not possible.
  Furthermore:
  - Renamed `-d/--dist` to `-D/--diffs`; `-d` is now shorthand for `--desc`
  - Added `-R/--diff-rate` (max. distance relative to pattern length)
  - *More variables/functions*:
    * `aligned_pattern`, `aligned_match`
    * `pattern`
    * `pattern_len`, `match_len`
    * `f:dist` renamed to `match_diffs`; `match_ins`, `match_del` and `match_subst` added to
      obtain insertions/deletions/substitutions
* *bugfixes* (minor bugs):
  - There was an inconsistency in how positions (ranges) of multiple pattern matches
    were reported (fuzzy matching only).
    Normally, only one hit was reported for each starting position, but if only the
    end of the range was needed (`f:end:...`, now `match_end(...)`),
    multiple hits with different alignments but the same starting position were reported.
    This did not affect the reporting of the *best* hit (the usual case).
  - Multi-line patterns in a FASTA file were not read correctly (newlines still in pattern) (#842835c)
* *ambiguous letters / sequence types*: The `B` and `Z` amino acid ambiguities
  are now recognized as well in addition to `X` (but not `U`, following
  rust-bio's `bio::alphabet::protein::iupac_alphabet`), see [this source file](src/cmd/find/ambig.rs)
  for details. The recognition of sequence types has been improved and the
  recognized type is reported for clarity.


#### Categorized counting (`count`)

With categorized counting, numeric categories are handled differetly:

The odd `n:<interval>:key` syntax was removed.
Instead, numeric variables are automatically treated as numeric
(categories are sorted accordingly). Alternatively, the `num(...)` variable
can be used to convert text keys to numeric.
Every unique value (with up to 6 decimal places) is treated as separate category.
Very large/small numbers are pretty-printed using the exponential notation.

To group numbers by interval, the new `bin(number, interval)` function can be used.

To be safe, the possible number of categories was limited and users
are informed about the `bin` function.

#### Random subsampling (`sample`)

* Selecting a fixed number of sequences (`-n`) now uses reservoir sampling instead of
  the previous "naive" (even though not incorrect) approach. The new algorithm
  does not require counting the total number of sequences beforehand, and therefore
  works with STDIN as well.
* A memory limit can be configured, above which the command switches to two-pass
  subsampling (no STDIN)

#### `split`

The new JavaScript engine allows assembling arbitrarily complex file paths,
making this command much more powerful.

#### `trim`

* The `trim` command now accepts comma-delimited lists of ranges in the form
`start1:end1,start2:end2`, etc., whereby the trimmed sub-ranges are concatenated.now
  Previously, only the `mask` command could handle multiple ranges.
* In the trim ranges, the start coordinates may now be greater than the end,
  which results in empty sequences. Previously, this resulted in the error
  *Range bound results in empty string*.

#### `view`

The `view` command obtained a nicely coloured visualization of the palettes
in `st view --list-pal`

* minor bugfix: lowercase letters are now coloured correctly

#### `stat`

The `stat` command is now a synonym of `st pass --to-tsv <fields>` and thus
accepts every possible variable/function

### Associated metadata

Associated 'lists' are now called 'metadata' (with `-m/--meta` flag instead of `-l/--list`).
The CLI descriptions should be more intuitively understandable.
The entries are retrieved with the `meta(...)` function.
Missing entries are handled with a separate `opt_meta(...)` function instead of a CLI flag.
`has_meta()` allows testing for the presence of a metadata entry.

The details of metadata parsing (in-order/synchronized parsing vs. unordered metadata stored in hash-map)
are now hidden from the user; the `-u/--unordered` flag was removed.
Problems only arise if there are duplicate IDs in the records and/or metadata.
Duplicates may only be detected with the unordered/hash-map parsing approach,
since the faster and more memory efficient in-order parsing approach does not
keep a record of already encountered IDs. The `--dup-ids` allows enforcing the use of
a hash map index from the start. In addition, the first 10k IDs are checked for duplicates
even with in-order parsing to detect problematic input.

### Header attributes

* `a:<name>` is now `attr(name)` or `opt_attr(name)`
* `has_attr(name)` tests for the presence of an attribute
* `-A/--append-attr` adds header attributes with minimum performance impact
* The key=value attribute format is now defined using `--attr-format` or
  the `ST_FORMAT` env variable instead of having two complicated options for it.
* Minor bugs fixed (#03ca54b).

### New behaviour with exclusive ranges

The and *trim*, *mask* accept an `-e/--exclusive` flag to exclude the start/end
bounds themselves from the range. The old behaviour was to also exclude the 
first/last position with *unbounded* ranges `:end` or `start:`. 
The new behaviour is to always include everything from the start/to the end
even with `-e/--exclusive`.
To exclude the start/end, specify `0:end` or `start:-1`.
See also the documentation on ranges.

### Highly customizable due to feature flags

Feature switches were added for every command, essentially allowing the assembly of
a totally customized binary.

For example, it is possible to have:

* `seqtool` with just two commands
* no JavaScript engine (without `expr` feature)
* a simpler regex engine (`regex-lite`) to save ~1.4MiB of binary size at the expense
  of fast searching
* a custom set of compression formats that `seqtool` can read/write.

### Other changes

* Missing values are now represented by `undefined` instead of empty strings.
  This is more intuitive and clear in some ambiguous situations.
  JavaScript `undefined` and `null` are also converted to `undefined`, e.g.
  when setting header attributes with `-a key={expression}`.
* CSV/TSV fields are *not* quoted/escaped anymore. If the separator (comma, tab, etc.)
  is found in one of the fields, this will lead to invalid output, and the user
  is responsible for avoiding this kind of problem.
* `--to-tsv/--to-csv` and the `stat` and `count` commands all consistently
  allow redirecting the output with `-o/--output` to either plain-text or compressed
  files. When reading, TSV/CSV fields are now expected in this order by default:
  `id,desc,seq`.
* Switched to Clap for option parsing
* BZIP2 reading/parsing is behind a feature flag, which is deactivated by default


## v0.3.0 (2018-08-14)

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
    by sequence quality as done by `fastq_filter` from USEARCH and VSEARCH
  - 454-style QUAL files can be read an written

* `--format` renamed to `--fmt`, and `--outformat` renamed to `--to`

**Bug fixes:**
- `seq_io` was updated, containing two small bug fixes. Both bugs were very
  unlikely to be hit and of low severity

## v0.2.3 (2018-05-01)

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

## v0.2.2 (2018-02-01)

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


## v0.2.1 (2018-01-15)

* recognize '.tsv' and uppercase extensions
* report names of missing files
* small performance improvements


## v0.2.0 (2018-01-10)

* added more advanced math expressions (ExprTk library)
* filter command
* renamed 'properties' to 'attributes'
* bug fixes, cleanups and more tests


v0.1.1: Initial release
