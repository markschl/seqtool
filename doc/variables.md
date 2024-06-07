# Variables/functions

*Seqtool* offers many variables/functions providing information about
the sequence records or the output of some commands.

The following variable categories are provided (amongst others):

* General properties of the sequence header (`id`, `desc`),
  the sequence (`seq`), input files (`filename`), etc.
* Sequence statistics such as the GC content (`gc` or `gc_percent`), etc.
* Access to *key=value* [attributes](attributes) in sequence headers (`attr(name)`, ...)
* Integration of metadata from [delimited text files](meta) (`meta(field)`, ...)
* Some commands provide the results of some calculations in the form of variables/functions
  ([find](find), [unique](unique), [sort](sort), [split](split))

**[Complete variable/function reference](var_reference)**


## Use in *seqtool* commands

Variables/functions are usually written in curly braces: `{variable}`,
although the braces can be omitted in some cases (see [below](#use-of-braces)).

The following command recodes IDs to `seq_1`, `seq_2`, `seq_3`, etc.:

```sh
st set -i seq_{num} seqs.fasta > renamed.fasta
```

The **[sort](sort)**, **[unique](unique)** and **[count](count)** use variables/functions
for categorization.

Example:

```sh
st sort seqlen input.fasta > length_sorted.fasta
```

The `{braces}` notation is only necessary for composed keys such as `{id}_{desc}`.

The **[trim](trim)** and **[mask](mask)** commands accept ranges or even lists of ranges
in the form of variables.

### Header attributes

Variables/functions are needed for composing new [header attributes](attributes).

```sh
st find PATTERN -a '{match_range}' input.fasta > with_range.fasta
```

```
>id1 3:10
SEQUENCE
>id2 5:12
SEQUENCE
(...)
```

### Expressions

All variables are available in [expressions](expressions), which usually need to
be in `{braces}`, except for the [filter](command).

Example: calculating the fraction of ambiguous bases for each sequence:

```sh
st stat '{ 1 - charcount("ATGC")/seqlen }'
```

```
id1	1
id2	0.99
id3	0.95
id4	1
...
```

## Delimited text output

Variables/functions are used to define the content of [delimited text files](pass).

This example searches a sequence ID for a string preceding the dot `.`
using a regular expression, and returns the matched text as TSV:

```sh
st find -ir '[^.]+' seqs.fasta --to-tsv 'id,match,seq' > out.tsv
```

`out.tsv`

```
seq1.suffix123	seq1	SEQUENCE`
seq2.suffix_abc	seq2	SEQUENCE`
...
```

> As with sort/unique/count keys, `{braces}` are not needed, unless a field is composed
> mixed text and/or other variables.

## Use of braces

The braced `{variable}` notation is *always* necessary:

* when setting/composing [attributes](attributes) with `-a/--attr key=value`
* if variables/functions are mixed with plain text text and/or other other variables
* in [set](set), output paths in [split](split), text replacements in [find](find)
* with JavaScript [expressions](expressions)

The braces *can optionally* be omitted if only a *single* variable/function
is used as:

* [sort](sort), [unique](unique) and [count](count) key: `st sort seq input.fasta`
* range bound in [trim](trim), [mask](mask): `st trim 'attr(start):' input.fasta`
* delimited text field: `st pass input.fasta --to-tsv id,desc,seq`
