Masking ranges are 1-based, using negative numbers means that the number is
relative to the sequence end (see [the explanation of ranges](ranges)
with basic examples).
A comma delimited list of ranges can be supplied, which may contain
variables, or the [whole range may be a variable](find#variables).

```bash
st find -r -a rng={f:drange:all} [AG]GA seqs.fa \
  | st mask a:rng
```
