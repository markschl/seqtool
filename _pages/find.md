Search for pattern(s) in sequences or sequene headers for record filtering,
pattern replacement or passing hits to next command

```
Usage: st find [OPTIONS] <PATTERNS> [INPUT]...

Arguments:
  <PATTERNS>  Pattern string or 'file:<patterns.fasta>'

Options:
  -h, --help  Print help

Where to search (default: sequence):
  -i, --id    Search / replace in IDs instead of sequences
  -d, --desc  Search / replace in descriptions

Search options:
  -D, --max-diffs <N>      Return pattern matches up to a given maximum edit
                           distance of N differences (= substitutions,
                           insertions or deletions). Residues that go beyond the
                           sequence (partial matches) are always counted as
                           differences. [default: pefect match]
  -R, --max-diff-rate <R>  Return of matches up to a given maximum rate of
                           differences, that is the fraction of divergences
                           (edit distance = substitutions, insertions or
                           deletions) divided by the pattern length. If
                           searching a 20bp pattern at a difference rate of 0.2,
                           matches with up to 4 differences (see also
                           `-D/--max-diffs`) are returned. [default: pefect
                           match]
  -r, --regex              Interpret pattern(s) as regular expression(s). All
                           *non-overlapping* matches in are searched in headers
                           or sequences. The regex engine lacks some advanced
                           syntax features such as look-around and
                           backreferences (see https://docs.rs/regex). Capture
                           groups can be extracted by functions such as
                           `match_group(number)`, or `match_group(name)` if
                           named: `(?<name>)` (see also `st find --help-vars`)
      --in-order           Report hits in the order of their occurrence instead
                           of sorting by distance. Note that this option only
                           has an effect with `-D/--max-dist` > 0, otherwise
                           matches are always reported in the order of their
                           occurrence
  -t, --threads <N>        Number of threads to use for searching [default: 1]
      --no-ambig           Don't interpret DNA ambiguity (IUPAC) characters
      --algo <NAME>        Override decision of algorithm for testing
                           (regex/exact/myers/auto) [default: auto]
      --gap-penalty <N>    Gap penalty to use for selecting the the optimal
                           alignment among multiple alignments with the same
                           starting position and the same edit distance. The
                           default penalty of 2 selects hits that don't have too
                           InDels in the alignment. A penalty of 0 is not
                           recommended; due to how the algorithm works, the
                           alignment with the leftmost end position is chosen
                           among the candidates, which often shows deletions in
                           the pattern. Penalties >2 will further shift the
                           preference towards hits with substitutions instead of
                           InDels, but the selection is always done among hits
                           with the same (lowest) edit distance, so raising the
                           gap penalty will not help in trying to enfoce
                           ungapped alignments (there is currently no way to do
                           that) [default: 2]

Search range:
      --rng <RANGE>          Search within the given range ('start:end',
                             'start:' or ':end'). Using variables is not
                             possible
      --max-shift-start <N>  Consider only matches with a maximum of <N> letters
                             preceding the start of the match (relative to the
                             sequence start or the start of the range `--rng`)
      --max-shift-end <N>    Consider only matches with a maximum of <N> letters
                             following the end of the match (relative to the
                             sequence end or the end of the range `--rng`)

Search command actions:
  -f, --filter          Keep only matching sequences
  -e, --exclude         Exclude sequences that matched
      --dropped <FILE>  Output file for sequences that were removed by
                        filtering. The output format is (currently) the same as
                        for the main output, regardless of the file extension
      --rep <BY>        Replace by a string, which may also contain
                        {variables/functions}
```

[See this page](opts) for the options common to all commands.

## Contents

* [Searching in headers](#searching-in-headers)
* [Searching in sequences](#searching-in-sequences)
* [Multiple patterns](#multiple-patterns)
* [Selecting other hits](#selecting-other-hits)
* [Replacing matches](#replacing-matches)
* [Variables/functions provided by the 'find' command](#variables/functions-provided-by-the-'find'-command)

## Details
### Searching in headers

Specify `-i/--id` to search in sequence IDs (everything before the first space)
or `-d/--desc` to search in the description part (everything *after* the space).

Example: selectively return sequences that have `label` in their description
(filtering with the `-f/--filter` flag):

```sh
st find -df 'label' gb_seqs.fasta
```

> *Note*: use `--dropped <not_matched_out>` to write unmatched sequences to 
> another file.

Often, searching in headers requires a regular expression (`-r/--regex`).
The following example extracts Genbank accessions from sequence headers that follow
the old-style Genbank format:

```sh
st find -ir "gi\|\d+\|[a-z]+\|(?<acc>.+?)\|.*" gb_seqs.fasta -a 'acc={match_group(acc)}'
```

```
>gi|1031916024|gb|KU317675.1| acc=KU317675.1
SEQUENCE
(...)
```

> You can use online tools such as https://regex101.com to build and debug your
> regular expression

> *Note:* replacing the whole header with the accession would be another
> (probably faster) approach, see the [replace](replace) command.


### Searching in sequences

Without the `-i` or `-d` flag, the default mode is to search in the sequence.
The pattern type is automatically recognized and usually reported to avoid
problems:

```sh
st find -f AATGRAAT seqs.fasta > filtered.fasta
```

```
Note: the sequence type of the pattern was determined as 'dna' (with ambiguous letters). If incorrect, please provide the correct type with `--seqtype`. Use `-q/--quiet` to suppress this message.
```

`R` stands for `A` or `G`. *Seqtool* recognizes the IUPAC ambiguity codes for
[DNA/RNA](https://iubmb.qmul.ac.uk/misc/naseq.html#500) and
[proteins](https://iupac.qmul.ac.uk/AminoAcid/A2021.html#AA212)
(with the exception of U = Selenocysteine).


**⚠** Matching is asymmetric: `R` in a search pattern matches [`A`, `G`, `R`]
in sequences, but `R` in a sequence will only match ambiguities sharing the same
set of bases (`R`, `V`, `D`, `N`) in the pattern. This should prevent false
positive matches in sequences with many ambiguous characters.


#### Approximate matching

*Seqtool* can find patterns with mismatches or insertions/deletions
(up to a given [edit distance](https://en.wikipedia.org/wiki/Edit_distance))
using the `-D/--diffs` argument. Alternatively, use `-R/--diff-rate` to
specify a distance limit relative to the length of the pattern
(in other words, an "error rate").

In this example, the edit distance and range of the best match are saved
into [header attributes](attributes) (or `undefined` if not found):

```sh
st find -D 2 AATGRAAT seqs.fasta -a d='{match_diffs}' -a rng='{match_range}'
```

```
>seq1 d=1 rng=3:11
GGAACGAAATATCAGCGATCC
>seq2 d=undefined rng=undefined
TTATCGAATATGAGCGATCG
(...)
```

In case of multiple hits, the second best hit can be returned by using
`{match_diffs(2)}` or `{match_range(2)}`, etc.


Use `--in-order` to report hits in order from left to right instead.

> *Note:* Approximative matching is done using [Myers](https://doi.org/10.1145/316542.316550)
> bit-parallel algorithm, which is very fast with short patterns and reasonably
> short sequences. It may not be the fastest solution if searching in large
> genomes.
> 
> Recognizing adapter or primers should be very fast.
> Further speedups can be achieved by multithreading (`-t`) and
> restricting the search range (`--rng`).

> *Note 2*: To report all hits below the given distance threshold 
> *in order of occurrence* instead of *decreasing distance*, specify `--in-order`.


### Multiple patterns

The *find* command supports searching for several patterns at once.
They have to be supplied in a separate FASTA file (`file:path`).
The best matching pattern with the smallest edit distance is always reported first.

The following example de-multiplexes sequences amplified with different forward
primers and then uses [trim](trim) to remove the primers, and finally distributes
the sequences into different files named by the forward primer ([split](split)).

<table>
<tr><th>

`primers.fasta`

</th></tr>
<tr><td>

```
>prA
PRIMER
>prB
PRIMER
```

</td></tr>
</table>


```sh
st find file:primers.fasta -a primer='{pattern_name}' -a end='{match_end}' sequences.fasta |
    st trim -e '{attr(end)}:' | 
    st split -o '{attr(primer)}'
```

<table>
<tr><th>prA.fasta </th><th>prB.fasta</th><th>undefined.fasta</th></tr>
<tr>
<td>

```
>id1 primer=prA end=22
SEQUENCE
>id4 primer=prA end=21
SEQUENCE
(...)
```

</td>
<td>

```
>id2 primer=prB end=20
SEQUENCE
>id3 primer=prB end=22
SEQUENCE
(...)
```

</td>
<td>

```
>id5 primer=undefined end=undefined
UNTRIMMEDSEQUENCE
(...)
```

> *Note:* no primer, sequence **not** trimmed since `end=undefined` (see [ranges](ranges)).

</td>
</tr>
</table>


### Selecting other hits

The find command is very versatile thanks to the large number of variables/functions
that provide information about the search results (see [variable reference](#variable-function-reference)).


For instance, the best hit from the *second best* matching pattern can be selected using
`{match_range(1, 2)}`.

It is also possible to return a comma-delimited list of matches, e.g.:
`{match_range(all)}`. See the [mask](mask) command for an example on how this could be useful.


### Replacing matches

Hits can be replaced by other text (`--repl`). Variables are allowed
as well (in contrast to the *replace* command). Backreferences to regex groups
(e.g. `$1`) are not supported like the *replace* command does.
Instead, they can be accessed using variables (`match_group()`, etc.)
### Variables/functions provided by the 'find' command
The find command provides many variables/functions to obtain information about the pattern matches. These are either written to header attributes (`-a/--attr`) or CSV/TSV fields (e.g. `--to-tsv ...`). See also examples section below.

| | |
|-|-|
| `match`<br />`match(hit)`<br />`match(hit, pattern)` | The text matched by the pattern. With approximate matching (`-D/--diffs` \> 0), this is the match with the smallest edit distance or the leftmost occurrence if `--in-order` was specified. With exact/regex matching, the leftmost hit is always returned. In case of multiple patterns in a pattern file, the best hit of the best-matching pattern is returned (fuzzy matching), or the first hit of the first pattern with an exact match.\<br /\>`match(hit) returns the matched text of the given hit number, whereas `match(all)` or `match('all') returns a comma-delimited list of all hits. These are either sorted by the edit distance (default) or by occurrence (`--in-order` or exact matching).\<br /\>`match(1, 2)`, `match(1, 3)`, etc. references the 2nd, 3rd, etc. best matching pattern in case multiple patterns were suplied in a file (default: hit=1, pattern=1)." |
| `aligned_match`<br />`aligned_match(hit)`<br />`aligned_match(hit, rank)` | Text match aligned with the pattern, including gaps if needed. |
| `match_start`<br />`match_start(hit)`<br />`match_start(hit, pattern)` | Start coordinate of the first/best match. Other hits/patterns are selected with `match_start(hit, [pattern])`, for details see `match` |
| `match_end`<br />`match_end(hit)`<br />`match_end(hit, pattern)` | Start of the first/best match relative to sequence end (negative coordinate). Other hits/patterns are selected with `match_neg_start(hit, [pattern])`, for details see `match`. |
| `match_neg_start`<br />`match_neg_start(hit)`<br />`match_neg_start(hit, pattern)` | End of the first/best match relative to sequence end (negative coordinate). Other hits/patterns are selected with `match_neg_end(hit, [pattern])`, for details see `match`. |
| `match_neg_end`<br />`match_neg_end(hit)`<br />`match_neg_end(hit, pattern)` | End coordinate of the first/best match. Other hits/patterns are selected with `match_end(hit, [pattern])`, for details see `match` |
| `match_len`<br />`match_len(hit)`<br />`match_len(hit, rank)` | Length of the match |
| `match_range`<br />`match_range(hit)`<br />`match_range(hit, pattern)`<br />`match_range(hit, pattern, delim)` | Range (start:end) of the first/best match. Other hits/patterns are selected with `match_range(hit, [pattern])`, for details see `match`. The 3rd argument allows changing the range delimiter, e.g. to '-'. |
| `match_group(group)`<br />`match_group(group, hit)`<br />`match_group(group, hit, pattern)` | Text matched by regex match group of given number (0 = entire match) or name in case of a named group: `(?\<name\>...)`. The hit number (sorted by edit distance or occurrence) and the pattern number can be specified as well (see `match` for details). |
| `match_grp_start(group)`<br />`match_grp_start(group, hit)`<br />`match_grp_start(group, hit, pattern)` | Start coordinate of the regex match group 'group' within the first/best match. See 'match_group' for options and details. |
| `match_grp_end(group)`<br />`match_grp_end(group, hit)`<br />`match_grp_end(group, hit, pattern)` | End coordinate of the regex match group 'group' within the first/best match. See 'match_group' for options and details. |
| `match_grp_neg_start(group)`<br />`match_grp_neg_start(group, hit)`<br />`match_grp_neg_start(group, hit, pattern)` | Start coordinate of regex match group 'group' relative to the sequence end (negative number). See 'match_group' for options and details. |
| `match_grp_neg_end(group)`<br />`match_grp_neg_end(group, hit)`<br />`match_grp_neg_end(group, hit, pattern)` | Start coordinate of regex match group 'group' relative to the sequence end (negative number). See 'match_group' for options and details. |
| `match_grp_range(group)`<br />`match_grp_range(group, hit)`<br />`match_grp_range(group, hit, pattern)`<br />`match_grp_range(group, hit, pattern, delim)` | Range (start-end) of regex match group 'group' relative to the sequence end. See 'match_group' for options and details. The 4th argument allows changing the range delimiter, e.g. to '-'. |
| `match_diffs`<br />`match_diffs(hit)`<br />`match_diffs(hit, pattern)` | Number of mismatches/insertions/deletions of the search pattern compared to the sequence (corresponds to edit distance). Either just `match_diffs` for the best match, or `match_diffs(h, [p])` to get the edit distance of the h-th best hit of the p-th pattern. `match_diffs('all', [p]) will return a comma delimited list of distances for all hits of a pattern. |
| `match_diff_rate`<br />`match_diff_rate(hit)`<br />`match_diff_rate(hit, pattern)` | Number of insertions in the sequence compared to the search pattern. Proportion of differences between the search pattern and the matched sequence, relative to the pattern length. See `match_diffs` for details on hit/pattern arguments. |
| `match_ins`<br />`match_ins(hit)`<br />`match_ins(hit, pattern)` | Number of insertions in the matched sequence compared to the search pattern. |
| `match_del`<br />`match_del(hit)`<br />`match_del(hit, pattern)` | Number of deletions in the matched text sequence to the search pattern. |
| `match_subst`<br />`match_subst(hit)`<br />`match_subst(hit, pattern)` | Number of substitutions (non-matching letters) in the matched sequence compared to the pattern |
| `pattern_name`<br />`pattern_name(rank)` | Name of the matching pattern (patterns supplied with `file:patterns.fasta`). In case a single pattern was specified in the commandline, this will just be `\<pattern\>`. `pattern_name(rank)` selects the n-th matching pattern, sorted by edit distance and/or pattern number (depending on `-D/-R` and `--in-order`). |
| `pattern`<br />`pattern(rank)` | The best-matching pattern sequence, or the n-th matching pattern if `rank` is given, sorted by edit distance or by occurrence (depending on `-D/-R` and `--in-order`). |
| `aligned_pattern`<br />`aligned_pattern(hit)`<br />`aligned_pattern(hit, rank)` | The aligned pattern, including gaps if needed. Regex patterns are returned as-is. |
| `pattern_len`<br />`pattern_len(rank)` | Length of the matching pattern (see also `pattern`). For regex patterns, the length of the complete regular expression is returned. |
#### Examples
Find a primer sequence with up to 2 mismatches (`-d/--dist``) and write the match range and the mismatches ('dist') to the header as attributes. The result will be 'undefined' (=undefined in JavaScript) if there are > 2 mismatches:
```sh
st find -d 2 CTTGGTCATTTAGAGGAAGTAA -a rng={match_range} -a dist={match_diffs} reads.fasta
```
```
>id1 rng=2:21 dist=1
SEQUENCE
>id2 rng=1:20 dist=0
SEQUENCE
>id3 rng= dist=
SEQUENCE
(...)
```
Find a primer sequence and if found, remove it using the 'trim' command, while non-matching sequences are written to 'no_primer.fasta':
```sh
st find -f -d 2 CTTGGTCATTTAGAGGAAGTAA --dropped no_primer.fasta -a end={match_end} reads.fasta |
   st trim -e '{attr(match_end)}..' > primer_trimmed.fasta
```
Search for several primers with up to 2 mismatches and write the name and mismatches of the best-matching primer to the header:
```sh
st find -d 2 file:primers.fasta -a primer={pattern_name} -a dist={match_diffs} reads.fasta
```
```
>id1 primer=primer_1 dist=1
SEQUENCE
>id1 primer=primer_2 dist=0
SEQUENCE
>id1 primer= dist=
SEQUENCE
(...)
```
