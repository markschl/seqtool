
## v0.4.0 (pre-release)

This release comes with substantial improvements and lots of internal code improvements.

First of all, we report the bugs :/

### Important bugfixes 🐞

Aside from minor bugs (listed further down), a few more important bugs were discovered
in *seqtool* v0.3.0, and are now fixed in v.0.4.0.

* Some multi-member GZIP and BZIP2 files were not read completely (#bc27f91).
  *seqtool* v0.3.0 stops parsing early, leading in truncated input.
  For example, `st count seqs.fastq.gz` may report a smaller sequence count, while
  `zcat seqs.fastq.gz | st count` reads the correct total count.
* Searching (*find*) and replacing (*replace*) in **multi-line** (wrapped) FASTA sequences
  lead to incorrect results
  - *find*: `-f/--filter` wrongly returns one or several sequences *after* a sequence match,
    even if they don't contain the searched pattern. Match coordinates (`f:start`, `f:range`, etc.)
    are also wrong (shifted).
  - *replace*: some sequence lines can be duplicated in the output, leading to "inflated"
    sequences, which in the worst case can lead to huge output files.
  The bug was implicitly fixed while refactoring (#03ca54b).
* In the *find* command, `--max-shift-r/l` (now `--max-shift-start/end`) did not
  play together with `-f/--filter` or `-e/--exclude`, records were always returned
  if *any* match was found, irrespective of their position in the sequence.

### New syntax for variables/functions

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
huge collections.

For efficient de-replication, the *seqhash* and *seqhash_both* variables/functions
were introduced.

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
`start1..end1,start2..end2`, etc., whereby the trimmed sub-ranges are concatenated.now
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

* Missing values are now represented by `N/A` instead of empty strings.
  This is more intuitive and clear in some ambiguous situations.
  JavaScript `undefined` and `null` are also converted to `N/A`, e.g.
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
