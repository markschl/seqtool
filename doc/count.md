By default, the count command will return the global count for all files in the
input:

```bash
seqtool count *.fastq
```

```
10648515
```

If the count for each file is needed, use the `filename` variable:

```bash
seqtool count -k filename *.fastq
```
```
file1.fastq    6474547
file2.fastq    2402290
file3.fastq    1771678
```

It is possible to use multiple keys. Consider the [example for the find
command](find#multiple-patterns) where the primer names and number of mismatches are
annotated as attributes. Now, the mismatch distribution for each primer
can be analysed:

```bash
seqtool count -k {a:f_primer} -k n:{a:f_dist} seqs.fa
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
N/A	5029
```

If primers on both ends were searched, it might make sense to use a
[math expression](variables#math-expressions) to get the sum of distances
for both primers.

```bash
seqtool count -k {a:f_primer} -k {a:r_primer} -k "n:{{a:f_dist + a:r_dist}}" primer_trimmed.fq.gz
```
```
f_primer1	r_primer1	0	3457490
f_primer1	r_primer1	1	491811
f_primer1	r_primer1	2	6374
f_primer1	r_primer1	3	420
f_primer1	r_primer1	4	10
(...)
```

The curly braces are actually only needed if a string of multiple
variables and/or text is composed. The `n:` prefix tells the tool that
the distance is numeric, which is useful for correct sorting.

With numeric keys, it is possible to summarize over intervals, add
a `n:<interval>` prefix. This example shows the GC content
summarized over 10% windows:

```bash
seqtool count -k n:10:{s:gc} seqs.fa
```
```
(20,30]	2
(30,40]	15
(40,50]	193
(50,60]	984
(60,70]	7
```

The intervals (start,end] are open at the start and
closed at the end, meaning that
start <= value < end.
