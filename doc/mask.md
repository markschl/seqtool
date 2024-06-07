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
