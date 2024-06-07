Soft or hard mask sequence ranges

```
Usage: st mask [OPTIONS] <RANGES> [INPUT]...

Options:
  -h, --help  Print help

'Mask' command options:
      --hard <CHAR>  Do hard masking instead of soft masking, replacing
                     everything in the range(s) with the given character
      --unmask       Unmask (convert to uppercase instead of lowercase)
  -e, --exclusive    Exclusive range: excludes start and end positions from the
                     masked sequence. In the case of unbounded ranges (`start:`
                     or `:end`), the range still extends to the complete end or
                     the start of the sequence
  -0, --zero-based   Interpret range as 0-based, with the end not included
  <RANGES>       Range in the form 'start:end' or 'start:' or ':end', The range
                 start/end may be defined by varialbes/functions, or the
                 varialbe/function may contain a whole range
```

[See this page](opts) for the options common to all commands.

## Details
Masking ranges are 1-based, using negative numbers means that the number is
relative to the sequence end (see [the explanation of ranges](ranges)).

A comma delimited list of ranges can be supplied, which may contain
variables, or the [whole range may be a variable](find#variables).

```sh
st find -r -a rng='{match_range(all)}' '[AG]GA' input.fasta \
  | st mask 'attr(rng)'
```

Possible output:

```
>seq464 rng=6:8,14:16
AGTTAagaCTTAAggaT
```
