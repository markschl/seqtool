This command is equivalent to 'seqtool pass --to-txt <stats>', but
variable prefixes (s:) are not necessary. Example:

```bash
seqtool stat seqlen,gc seqs.fa
```

Example output:

```
seq1	291	50.51546391752577
seq2	297	57.57575757575758
...
```