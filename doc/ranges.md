# Explanation of ranges

Ranges in st are used by commands like [trim](trim) and [mask](mask),
and returned by the [find](find) command.

They look like this: `<start>..<end>`. Open ranges are possible: `<start>..`
(trims only at the start, including the end of the sequence) and `..<end>`
(trims only at the end, including the start of the sequence to `<end>`).
The coordinates are 1-based, meaning that `1` denotes the first character
(unless `-0` is used). It is also possible to use negative numbers, which
will tell the tool to count from the end of the sequence:

<pre>
sequence:    A   T  <b>G   C   A   T</b>   G   C
from start:  1   2  <b>3   4   5   6</b>   7   8
from end:   -8  -7 <b>-6  -5  -4  -3</b>  -2  -1
0-based:     0   1  <b>2   3   4   5</b>   6   7
</pre>

All of the following commands will trim to the range shown in bold:

```bash
# 1-based positive
st trim "3..6" seqs.fa

# 1-based negative
# space before range and quote necessary
st trim " -6..-3" seqs.fa.

# 0-based
st trim 2..6 seqs.fa
```

**Note**: There is a problem with ranges starting with a negative number
being interpreted as command line arguments. However, insertion of a
space before the minus sign will work.


#### Empty ranges

Note that ranges of zero length are only possible if
the start is greater than the end, e.g.: `5..4`.
