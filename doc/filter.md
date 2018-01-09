
### Examples:

Removing sequences shorter than 100 bp:

```bash
seqtool filter 's:seqlen >= 100' input.fa > filtered.fa
```

Removing DNA sequences with more than 10% of ambiguous bases:

```bash
seqtool filter 's:count:ATGC / s:seqlen >= 0.1' input.fa > filtered.fa
```

Quick and easy way to select certain sequences (for more advanced
filtering using lists, see below):

```bash
seqtool filter ".id == 'id1' or .id like 'AB*'" input.fa > filtered.fa
```

Note the `.` before the variable name. This indicates that this is a 
[string comparison](variables#string-variables).

Keeping only sequences with less than five primer mismatches (stored in the
`f_dist` attribute, see [example for the find command](find#multiple-patterns)),
and which are also long enough:

```bash
seqtool filter 'a:f_dist < 5 and s:seqlen > 100' primer_search.fa > filtered.fa
```

Fairly advanced expressions, even small scripts are possible.
An overview of the syntax can be found 
[here](https://github.com/ArashPartow/exprtk/blob/f32d2b4bbb640ea4732b8a7fce1bd9717e9c998b/readme.txt#L44).


**Note**: This command is only available if compiling with the `exprtk` feature.
(`cargo build --release --features=exprtk`) to activate usage of the
[ExprTk](http://www.partow.net/programming/exprtk/) C++ library.
However, the provided binaries include this feature by default.

### Undefined values

Undefined variables can occur if a record could not
be found in an [associated list](lists), an [attribute](attributes) was not present
in the header, or the requested match/match group was not found by
the [find](find) command. Whether a variable is defined can be checked
using the `def()` function. It returns `true` if the given variable is defined for 
a given sequence.

Example for retrieving sequences stored in a list of IDs
(see also [here](lists#filtering-given-an-id-list)):


```bash
seqtool filter -uml id_list.txt 'def(l:1)' seqs.fa > in_list.fa
```

Note that **empty strings** are also treated as undefined. Consider this
FASTA file:

```
>id1
SEQUENCE
>id2 value=20
SEQUENCE
>id3 value=
SEQUENCE
```

The following command does an additional check if the attribute `value` 
is defined or not:

```bash
seqtool filter 'def(a:value) and a:value > 5' seqs.fa
```

Output:

```
>id2 value=20
SEQ
```

Since seq3 has an empty `value` attribute, it is also removed by filtering.

**Also note**: Undefined variables are represented by `NaN`. In ExprTk,
comparisons to `NaN` always result in `false`. Therefore, the check using
`def()` is not actually necessary in this case because `a:value > 5` would
anyway return `false` for seq1 and seq3.

