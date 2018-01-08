The trim ranges are 1-based, using negative numbers means that the number is relative to the sequence end (see [the explanation of ranges](ranges)
with basic examples).

Example bash commands for removing primers from the ends:

```bash
f_primer=GATGAAGAACGYAGYRAA
r_primer=TCCTCCGCTTATTGATATGC

seqtool trim -- "${#f_primer} .. -${#r_primer}" input.fa > output.fa
```
**Note:** Since the last primer base should not be included, we use
the `-e/--exclude` option.


## Using variables

The command becomes very useful with variables. The following is equivalent
to [bedtools getfasta](http://bedtools.readthedocs.io/en/latest/content/tools/getfasta.html)
(note that the BED format is 0-based, thus the `-0` option):

```bash
seqtool trim -l coordinates.bed -0 {l:2}..{l:3} input.fa > output.fa
# instead of -0 we could also use a math expression:
seqtool trim -l coordinates.bed -0 '{{l:2 + 1}}..{l:3}' input.fa > output.fa
```

It is also possible to directly use the output of the [find](find) command,
e.g. if looking for primers:

_input.fa_ (N is a placeholder):

```
>id
NGATGAAGAACGYAGYRAANNNNNNNNNNNNNNNNNNNTCCTCCGCTTATTGATATGCN
```

Looking for primers, storing the result in attributes using the
`f:drange` variable (for dot range):

```bash
f_primer=GATGAAGAACGYAGYRAA
r_primer=TCCTCCGCTTATTGATATGC

seqtool find -d4  --rng ..23 -p f_end={f:end} -ad4 $f_primer input.fa \
  | seqtool find  --rng ' -23..' -p r_start={f:neg_start} -d4 $r_primer \
  > primer_search.fa
```

_primer\_search.fa:_

```
>id f_end=19 r_start=-21
NGATGAAGAACGYAGYRAANNNNNNNNNNNNNNNNNNNTCCTCCGCTTATTGATATGCN
```

Now we can use this range for trimming:

```bash
seqtool trim -e '{a:f_end}..{a:r_start}' primer_search.fa > no_primers.fa
```

_no\_primers.fa:_

```
>id f_end=19 r_start=-21
NNNNNNNNNNNNNNNNNNN
```
