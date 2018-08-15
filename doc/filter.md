
### Examples

Removing sequences shorter than 100 bp:

```bash
st filter "s:seqlen >= 100" input.fa > filtered.fa
```

Removing DNA sequences with more than 10% of ambiguous bases:

```bash
st filter "s:count:ATGC / s:seqlen >= 0.1" input.fa > filtered.fa
```

Quick and easy way to select certain sequences (for more advanced
filtering using lists, see below):

```bash
st filter ".id == 'id1' or .id like 'AB*'" input.fa > filtered.fa
```

Note the `.` before the variable name. This indicates that this is a
[string comparison](variables#string-variables).

Keeping only sequences with less than five primer mismatches (stored in the
`f_dist` attribute, see [example for the find command](find#multiple-patterns)),
and which are also long enough:

```bash
st filter "a:f_dist < 5 and s:seqlen > 100" primer_search.fa > filtered.fa
```

Fairly advanced expressions, even small scripts are possible.
An overview of the syntax can be found
[here](https://github.com/ArashPartow/exprtk/blob/f32d2b4bbb640ea4732b8a7fce1bd9717e9c998b/readme.txt#L44).


**Note**: This command is only available if compiling with the `exprtk` feature.
(`cargo build --release --features=exprtk`) to activate usage of the
[ExprTk](http://www.partow.net/programming/exprtk/) C++ library.
However, the provided binaries include this feature by default.

### Quality filtering

The `exp_err` statistics variable represents the total expected number of errors
in a sequence, as provided by the quality scores. [See here](pass#quality-scores)
for more information on reading them.

This example removes sequences with less than one expected error. The
output is the same as from `fastq_filter` [USEARCH](https://www.drive5.com/usearch/manual/cmd_fastq_filter.html).

```bash
st filter 's:exp_err >= 1' input.fq > filtered.fq
```

Normalization according to sequence length is easily possible with
a math formula (corresponding to `-fastq_maxee_rate` of USEARCH).

```bash
st filter 's:exp_err / s:seqlen >= 0.002' input.fq > filtered.fq
```

### Undefined (missing) values

Undefined variables can occur if a record could not
be found in an [associated list](lists), an [attribute](attributes) was not present
in the header, or the requested match/match group was not found by
the [find](find) command. Whether a variable is defined can be checked
using the `def()` function. It returns `true` if the given variable is defined for
a given sequence.

Example for retrieving sequences stored in a list of IDs
(see also [here](lists#filtering-given-an-id-list)):


```bash
st filter -uml id_list.txt "def(l:1)" seqs.fa > in_list.fa
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
st filter "def(a:value) and a:value > 5" seqs.fa
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
