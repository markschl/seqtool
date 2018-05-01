The find command allows searching for patterns in sequences or sequence headers.
It can be used for simple filtering, but results can also be returned by
using [the variables](#variables). Occurrences can be replaced.


### Exact searching

This is the simplest and also the fastest way of searching.
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
seqtool find -ir "gi\|\d+\|[a-z]+\|(?P<acc>.+?)\|.*" seqs.fa \
   --rep "{f:match::acc}" > seqs_accession.fa
```

Headers like this one: `>gi|1031916024|gb|KU317675.1|` are transformed
to `>KU317675.`

**Note:** Only ASCII is currently supported, unicode is not recognized
in regular expressions.

### Approximative searching

Approximative matching is particularly useful for finding primers and
adapters or other short patterns. The tool is not specialized for removing
adapters like other software, but still aims at being very fast and
useful for many purposes. **Note:** The length of search patterns for
approximative matching is limited to 64 characters, currently.

It is possible to search for all matches up to a maximum
[edit distance](https://en.wikipedia.org/wiki/Edit_distance)
(`-d/--dist` argument). By default, the best hit is reported first.
Example:

```bash
seqtool find -d 4 ATTAGCG seqs.fa \
     -a hit="{f:range}_(dist:{f:dist})" -a range="{m:range}"
```

Possible output:

```
>seq464 hit=2-9_(dist:1) matched=ATCAGCG
GGATCAGCGATCC
(...)
```

The second best hit can be returned by using `{f:range:2}` or `{f:match:2}`, etc.
Use `--in-order` to report hits in order from left to right instead.

Substantial speedups can be achieved by [restricting the search range](#restrict_search_range)
and by multithreading (`-t`).


#### Search performance

Approximative matching uses the fast bit-parallel algorithm by
[Myers](https://doi.org/10.1145/316542.316550).
The runtimes for searching two forward primers (22 bp) with up to 4 mismatches (`-d 4`)
in a [1.2 GB file](https://github.com/markschl/seqtool#performance)
vary depending on the options used. In the following table, different settings
as well as other tools are compared, using 1 or 4 threads or processes ('cores').

|                                                         | 1 core      | 4 cores     |
|---------------------------------------------------------|-------------|-------------|
| Find the position of best hit                           | 31.3s       | 7.73s       |
| Report first hit from left<sup>1</sup>                  | 31.1s       | 7.72s       |
| Best hit in range where the primer should occur<sup>2</sup>| 8.02s    | 2.34s       |
| Filter by occurrence (`-f`), no position determined     | 14.0s       | 3.24s       |
| Exact search (no mismatches/ambiguities)<sup>3</sup>    | 11.1s       | 2.57s       |
| [cutadapt](https://github.com/marcelm/cutadapt)         | 1min 16s    | 24.2s       |
| [AdapterRemoval](https://github.com/MikkelSchubert/adapterremoval)<sup>4</sup> | 35.0s| 8.75s |

<sup>1</sup> `--in-order` option. Normally, hits are sorted by decreasing
distance.

<sup>2</sup> `--rng ..25`

<sup>3</sup> Using the Two Way algorithm for comparison, not Myers

<sup>4</sup> Single-end mode


### Ambiguities

DNA (RNA) ambiguity codes according to the IUPAC nomenclature are accepted and
automatically recognised in search patterns. For Proteins, `X` is recognised as
wildcard for all amino acids. The molecule type is automatically determined
from the pattern. Use `-v` to show which search settings are being used. Example:

```bash
seqtool find -v file:primers.fasta -a primer={f:name} -a rng={f:range} input.fasta > output.fasta
```

```
primer1: DNA, search algorithm: Exact
primer2: DNA with ambiguities, search algorithm: Myers
Sorting by distance: true, searching start position: true
```

In case of wrongly recognised patterns, specify `--seqtype`. The tool is
very cautious and will warn about any inconsistencies between multiple patterns.

**Note:** Matching is asymmetric: `R` in a search pattern matches [`A`, `G`, `R`]
in sequences, but `R` in a sequence will only match ambiguities sharing the same
set of bases (`R`, `V`, `D`, `N`) in the pattern. This should prevent false
positive matches in sequences with many ambiguous characters.


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

Example output:

```
>seq1 f_primer=primer2 f_dist=1 second_best=primer1_(5)
SEQUENCE
```


### Restrict search range

It is possible to search only part of the target string by using `--rng`.
This can substantially speed up the search.


```bash
seqtool find -d6 --rng ..23 file:f_primers.fa seqs.fa \
    -p f_primer={f:name} > primer_search.fa
```

If the hit is known to occur at the start or end of the
search range, `--max-shift-l` and `--max-shift-r` can be
used to report only those hits.

### Replace matches

Hits can be replaced by other text (`--repl`). Variables are allowed
as well (in contrast to the *replace* command). Backreferences to regex groups
(e.g. `$1`) are not supported like the _replace_
command does. Instead, they can be accessed using variables
(`<variable>::<group>`)

### Variables

Variables for the matched sequence (`f:match`), coordinates
(`f:start`, `f:end`, `f:range`, etc.) are available.
Selecting another match than the best/first one is possible by adding
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
  | seqtool mask {a:rng}
```

Exmaple output:

```
>seq464 rng=6..8,14..16
AGTTAagaCTTAAggaT
```
