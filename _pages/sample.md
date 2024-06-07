Get a random subset of sequences; either a fixed number or an approximate
fraction of the input

```
Usage: st sample [OPTIONS] [INPUT]...

Options:
  -h, --help  Print help

'Sample' command options:
  -n, --num-seqs <N>    Randomly select a fixed number of sequences. In case
                        speed is important, consider -p/--prob. For lower memory
                        usage (but less speed), supply -2/--to-pass
  -p, --prob <PROB>     Instead of a fixed number, include each sequence with
                        the given probability. There is no guarantee about an
                        exact number of returned sequences, but the fraction of
                        returned sequences will be near the specified
                        probability
  -s, --seed <SEED>     Use a seed to make the sampling reproducible. Useful
                        e.g. for randomly selecting from paired end reads.
                        Either a number (can be very large) or an ASCII string,
                        from which the first 32 characters are used
  -2, --two-pass        Use two-pass sampling with -n/--num-seqs: (1) read all
                        files to obtain the total number of sequences, (2) read
                        again, and return the selected sequences. This uses less
                        memory, but does not work with STDIN and may be
                        especially slow with compressed files. Automatically
                        activated if the -M/--max-mem limit is reached
  -M, --max-mem <SIZE>  Maximum amount of memory to use for sequences. Either a
                        plain number (bytes) a number with unit (K, M, G, T)
                        based on powers of 2. This limit may be hit if a large
                        number of sequences is chosen (-n/--num-seqs). If
                        reading from a file (not STDIN), the program will
                        automatically switch to two-pass sampling mode.
                        Alternatively, conider using -p/--prob if the number of
                        returned sequences does not have to be exact [default:
                        5G]
```

[See this page](opts) for the options common to all commands.

