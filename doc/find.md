The find command allows searching for patterns in sequences or sequence headers.
It can be used for simple filtering, but results can also be returned by
using [the variables](#variables). Occurrences can be replaced.


### Exact searching

This is the most simple and also the fastest way of searching.
In the following example, a pattern
is searched and only hits matching sequences are returned (`-f`).
Unmatched sequences are written to a different file.

```bash
seqtool find -s AATG -f --dropped not_matched.fa seqs.fa
```

### Regular expressions

Especially useful when searching in sequence headers. Different regex
groups can be accessed as [output variables](#variables).

The following example extracts Genbank accessions from sequences in an old style
Genbank format by using a regex group named 'acc' and replaces the ID with
this group:

```bash
seqtool find -ir 'gi\|\d+\|[a-z]+\|(?P<acc>.+?)\|.*' seqs.fa \
   --rep '{f:match::acc}' > seqs_accession.fa
```

Headers like this one: `>gi|1031916024|gb|KU317675.1|` are transformed
to `>KU317675.`

**Note:** Only ASCII is currently supported, unicode is not recognized
in regular expressions.


### Approximative searching

Approximative matching is particularly useful for finding primers and
adapters or other short patterns.
The approach of seqtool aims to be somehow more general than
the one by the specialized tool [cutadapt](https://github.com/marcelm/cutadapt).

It is possible to search for all matches up to a maximum
[edit distance](https://en.wikipedia.org/wiki/Edit_distance)
(`-d/--dist` argument). By default, the best hit is reported first.
Example:

```bash
seqtool find -d 4 ATTAGCG seqs.fa \
     -p hit='{f:range}_(dist:{f:dist})' -p matched ='{m:match}'
```

Possible output:

```
>seq464 hit=2-9_(dist:1) matched=ATCAGCG
GGATCAGCGATCC
(...)
```

The second best hit can be returned by using `{f:range:2}` or `{f:match:2}`, etc...
Note that due to the way these algorithms work, many overlapping hits with different
distances to the sequence can be returned. Thus, the second best hit may overlap
with the best hit. Use `-g yes` to only report one hit per position. This is
off (`-g no`) by default since it can have a major performance impact. Only
if `--inorder` is used, it is on by default.

In the case of simple filtering by occurrence (`-e`/`-f`), this
is not required. However, if any variable with positional information is
used, this will impact performance.

Additional speedups can be achieved by [restricting the search range](#restrict_search_range) and multithreading (`-t`).

#### Algorithms and performance

The procedure involves searching for all hits up to the given edit distance
([Ukkonen, 1985](https://doi.org/10.1016/0196-6774(85)90023-9) or an accelerated version by [Myers](https://doi.org/10.1145/316542.316550)), implemented in
the [Rust-Bio](http://rust-bio.github.io/)
library. This gives the end positions of each hit. To obtain the starting
positions, a simple semi-global alignment is done.

The runtimes for searching two 5' primers in a [1.1 GB file](performance)
vary depending on the options used.

|                                                         | seqtool     | (4 threads) | cutadapt   |
|---------------------------------------------------------|-------------|-------------|------------|
| Find the position of the best hit in the whole sequence | 2min 20s  | 37.0s       |            |
| Search only in range where primer should occur (--rng) | 52.1s      | 13.5s      | 1min 18s* |
| Search whole sequence + filter by occurrence only (-f) | 53.0s      | 14.3s      |            |
| No matching of ambiguous bases (-a no) -> Myers algorithm | 1min 5s   | 16.7s      |            |
| Find the first hit. Merging overlapping hits makes this slower.  | 5min 14s  | 1min 23s    |            |

* Actually, cutadapt uses semi-global alignment with penalties for leading gaps,
which is different from manually restricting the search range.

**Note:** Ukkonen matching currently has a [bug](https://github.com/rust-bio/rust-bio/issues/117): matches starting at position 0 reports a wrong distance (dist + 1).
Until fixed, make sure to set `-d` high enough, and this should not be a problem.


### Ambiguities

DNA ambiguity codes according to the IUPAC nomenclature are accepted in patterns
and DNA sequences. This can have a performance impact because the Ukkonen
algorithm is always used in this case. Matching is asymmetric:
`R` in a search pattern is matched by [`A`, `G`, `R`] in sequences,
but `R` in a sequence will only match ambiguities sharing the same set of bases
(`R`, `V`, `D`, `N`) in the pattern. This should prevent false positive matches
in sequences with many `N`s.


### Multiple patterns

Several patterns can be searched in one command. They have to be supplied
in a separate FASTA file. The best matching pattern with the smallest edit
distance is always reported first. Other patterns are accessed using
`<variable>.<pattern_num>`:

```bash
seqtool find -d6 file:f_primers.fa seqs.fa \
    -p f_primer={f:name} -p f_dist={f:dist} \
    -p second_best={f:name.2}_({f:dist.2}) > primer_search.fa
```

Exmaple output:

```
>seq1 f_primer=primer2 f_dist=1 second_best=primer1_(5)
SEQUENCE
```


### Restrict search range

It is possible to search only part of the target
string. By using `--rng`. This can substantially speed up the search.


```bash
seqtool find -d6 --rng ..23 file:f_primers.fa seqs.fa \
    -p f_primer={f:name} > primer_search.fa
```

If the hit is known to occur at the start or end of the
search range, `--max-shift-l` and `--max-shift-r` can be
used to report only those hits.

### Replace matches

Hits can be replaced by other text. Variables are allowed
as well (in contrast to the *replace* command). It is possible
Backreferences to regex groups (e.g. `$1`) are not supported like the _replace_
command does. Instead, they can be accessed using variables
(`<variable>::<group>`)

### Variables

Variables for the matched sequence (`f:match`), coordinates
(`f:start`, `f:end`, `f:range`, etc.) are available.
Selecting another match than the first one is possible by adding
the match number after a colon (`:`): `f:range:2` will return
the second match.
Match groups of regular expressions can be
specified as well by the addition of a second colon:
`f:range::2` will select the second match group of the first match.
Even named groups are possible. In that case, specify the group name
instead of the index: `f:range::<group_name>`.

A more generalized scheme:

`m:<variable>.<pattern_rank>:<hit_num>:<match_group>`

This is admittedly quite complicated, but adds a lot of flexibility.

It is also possible to return all hits instead of a specific one
by using `f:<variable>:all`. This will return a comma delimited list.
The following command searches for all occurrences of a pattern
and converts them to lowercase:

```bash
seqtool find -r -p rng={f:drange:all} [AG]GA seqs.fa \
  | seqtool mask {p:rng}
```

Exmaple output:

```
>seq464 rng=6..8,14..16
AGTTAagaCTTAAggaT
```
