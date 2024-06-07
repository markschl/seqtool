Immagine this FASTA file (`input.fa`):

```
>seq1 group=1
SEQUENCE
>seq2 group=2
SEQUENCE
>seq3 group=1
SEQUENCE
```

```sh
st split -o "group_{attr(group)}.fa" input.fasta
```

This will create the files `group_1.fa` and `group_2.fa`. In more
complicated scenarios, variables may be combined for creating nested subfolders
of any complexity.

An example of de-multiplexing sequences by forward primer is found in the
documetation of the [find](find#multiple-patterns) command.
