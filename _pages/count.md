Count all records in the input (total or categorized by variables/functions)

```
Usage: st count [OPTIONS] [INPUT]...

Options:
  -h, --help  Print help

'Count' command options:
  -k, --key <KEY>
          Count sequences for each unique value of the given category. Can be a
          single variable/function such as 'filename', 'desc' or 'attr(name)',
          or a composed key such as '{filename}_{meta(species)}'. The `-k/--key`
          argument can be specified multiple times, in which case there will be
          multiple category columns, one per key
  -l, --category-limit <CATEGORY_LIMIT>
          Maximum number of categories to count before aborting with an error.
          This limit is a safety measure to prevent memory exhaustion. Usually,
          a very large number of categories is not intended and may happen if
          continuous numbers are not categorized with the `bin(<num>,
          <interval>)` function [default: 1000000]
```

[See this page](opts) for the options common to all commands.

## Details
## Counting the overall record number

By default, the count command returns the overall number of records in all
of the input (even if multiple files are provided):

```sh
st count *.fastq
```

```
10648515
```

## Categorized counting


Print record counts per input file:

<c>`st count -k path input.fasta input2.fasta input3.fasta`</c><r>
input.fasta   1224818
input2.fasta  573
input3.fasta  99186
</r>


If the record number is required for each file, use the `path` or `filename`
variable:

```sh
st count -k path *.fasta
```
```
file1.fasta    6470547
file2.fasta    24022
file3.fasta    1771678
```

Print the sequence length distribution:

```sh
st count -k seqlen input.fasta
```
```
102 1
105 2
106 3
(...)
```

It is possible to use multiple keys. Consider the
[primer finding example](find#multiple-patterns) where the primer names 
and number of mismatches are annotated as attributes.
Now, the mismatch distribution for each primer can be analysed:

```sh
st count -k 'attr(f_primer)' -k 'attr(f_dist)' seqs.fasta
```
```
primer1	0	249640
primer1	1	23831
primer1	2	2940
primer1	3	123
primer1	4	36
primer1	5	2
primer2	0	448703
primer2	1	60373
primer2	2	8996
primer2	3	691
primer2	4	34
primer2	5	7
primer2	6	1
undefined	5029
```

If primers on both ends were searched, it might make sense to use an
[expression](expressions) to get the sum of edit distances for both primers.

```sh
st count -k 'attr(f_primer)' -k 'attr(r_primer)' \
  -k '{ num(attr("f_dist")) + num(attr("r_dist")) }' primer_trimmed.fq.gz
```
```
f_primer1	r_primer1	0	3457490
f_primer1	r_primer1	1	491811
f_primer1	r_primer1	2	6374
f_primer1	r_primer1	3	420
f_primer1	r_primer1	4	10
(...)
```

> ⚠ [JavaScript expressions](expressions) always need to be enclosed in
> `{curly braces}`, while simple variables/functions only require this
>  [in some cases](variables). Also, attribute names need to be in double
>  or single quotes: `attr("f_dist")`.

> ⚠ The `f_dist` and `r_dist` attributes are numeric, but *seqtool* doesn't know
> that (see [below](#numbers-stored-as-text)), and the JavaScript expression would simply
> [concatenate them as strings](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Language_overview#strings)
> instead of adding the numbers up. Therefore we require the `num` function
> for conversion to numeric.

## Numeric keys

With numeric keys, it is possible to summarize over intervals using the 
`bin(number, interval)` function. Example summarizing the GC content:

```sh
st count -k '{bin(gc_percent, 10)}' seqs.fasta
```
```
(10, 15]    2
(15, 20]    9
(20, 25]    357
(25, 30]    1397
(30, 35]    3438
(35, 40]    2080
(40, 45]    1212
(45, 50]    1424
(50, 55]    81
```

The intervals `(start,end]` are open at the start and
closed at the end, meaning that
`start <= value < end`.

### Numbers stored as text

In case of a header attribute `attr(name)` or a value from
an associated list `meta(column)`, these are always interpreted
as text by default, unless the `num(...)` function is used,
which makes sure that the categories are correctly sorted:

```sh
st count -k 'num(attr(numeric_attr))' input.fasta
```
