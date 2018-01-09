# Variables

Many commands can use variables, and some of them will
also provide part of their output as variables. See below
for a list of global variables. They are normally written in
curly braces: `{variable}`. The following example recodes
sequence IDs to `seq_1/2/3...`:

```bash
seqtool set -i seq_{num} seqs.fa > renamed.fa
```

Besides `num`, there are many other variables that can be used
in any other command (see [below](#variables-available-to-all-commands)).
They are structured into different categories which all have a specific
prefix divided from the variable with a colon (`<prefix>:<varible>`).

* [attributes](attributes): `a:<name>`
* Data from [associated lists](lists): `l:<fieldname>` or `l:<column_index>`
* Sequence statistics: `s:<name>` (also available in dedicated [stat](stat) command)
* Variables provided by commands, currently: [find](find) (`f:`) and
  [split](split) (`split:`)

The prefix makes it possible to e.g. have list fields and attributes with the
same name.

**Note**  that the variable is written inbetween curly brackets: `{<a:otu>}`.
This is also required when using them in [attributes](#attributes).

### Writing to output

Variables provided by commands (and all others) can be written to the output
in two ways: [attributes](attributes) and [CSV/TXT output](converting).
This example uses regex matching:

```bash
seqtool find -ir '([^\.]+).*' seqs.fa -p id={f:match::1}
# returns `>seqname.1234 id=seqname`

seqtool find -ir '([^\.]+).*' seqs.fa --to-txt id,f:match::1,seq
# returns `seqname.1234 seqname SEQ`
# Note: curly brackets are not necessary here.
```

## Math expressions

Mathematical expressions are written with double curly brackets.
This example calculates the length of a match found by the _find_ command.

```bash
seqtool find -d3 GCATATCAATAAGCGGAGGA seqs.fa \
  -p match_len='{{f:end - f:start + 1}}'
```

If compiled with [ExprTk](http://www.partow.net/programming/exprtk/) support 
(which is the default for the provided binaries), filtering expressions
are also possible using the [filter](filter) command:

```bash
seqtool filter 's:seqlen >= 100' input.fa > filtered.fa
```

### String variables

ExprTk expressions can also handle strings. String variables have to be
explicitly marked as such using a preceding dot (`.variable`).

```bash
seqtool filter ".id == 'id1' or .id == 'id2'" input.fa > filtered.fa
```

