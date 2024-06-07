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
