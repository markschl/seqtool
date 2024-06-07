Concatenates sequences/alignments from different files

```
Usage: st concat [OPTIONS] [INPUT]...

Options:
  -h, --help  Print help

'Concat' command options:
  -n, --no-id-check      Don't check if the IDs of the records from the
                         different files match
  -s, --spacer <SPACER>  Add a spacer of <N> characters inbetween the
                         concatenated sequences
  -c, --s-char <S_CHAR>  Character to use as spacer for sequences [default: N]
  -Q, --q-char <Q_CHAR>  Character to use as spacer for qualities. Defaults to a
                         phred score of 41 (Illumina 1.8+/Phred+33 encoding,
                         which is the default assumed encoding) [default: J]
```

[See this page](opts) for the options common to all commands.

