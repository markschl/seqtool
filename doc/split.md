Immagine this FASTA file (`input.fa`):

```
>seq1;group=1
SEQUENCE
>seq2;group=2
SEQUENCE
>seq3;group=1
SEQUENCE
```

```bash
seqtool split -o "group_{a:group}.fa" --adelim ";" input.fa
```

This will create the files `group_1.fa` and `group_2.fa`. In more
complicated scenarios, variables may be combined for creating nested subfolders
of any complexity.
