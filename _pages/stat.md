Return per-sequence statistics as tab delimited list

```
Usage: st stat [OPTIONS] <VAR> [INPUT]...

Options:
  -h, --help  Print help

'Stat' command options:
  <VAR>  Comma delimited list of statistics variables
```

[See this page](opts) for the options common to all commands.

## Details
`st stat <variables>` is a shorter equivalent of `st pass --to-tsv id,<variables>`.

Example:

```sh
st stat seqlen,gc seqs.fasta
```

Example output:

```
seq1	291	50.51546391752577
seq2	297	57.57575757575758
...
```
