# Explanation of ranges

Ranges in seqtool are used by commands like [trim](trim.html) and [mask](mask.html),
and returned by the `find` command.

They look like this: `<start>..<end>`. Open ranges are possible: `<start>..`
(trims only at the start, including the end of the sequence) and `..<end>`
(trims only at the end, including the start of the sequence to `<end>`).
The coordinates are 1-based, meaning that `1` denotes the first character
(unless `-0` is used). It is also possible to use negative numbers, which
will tell the tool to count from the end of the sequence:

<pre>
sequence:    A   T  <b>G   C   A   T</span>   G   C<
from start:  1   2  <b>3   4   5   6</span>   7   8
from end:   -8  -7 <b>-6  -5  -4  -3</span>  -2  -1
0-based:     0   1  <b>2   3   4   5</span>   6   7
</pre>

In this example, the following commands will trim output the range printed in bold
letters.

```bash
# 1-based positive
seqtool trim '3..6' seqs.fa

# 1-based negative
# space before range and quotes necessary due to a bug
seqtool trim ' -6..-3' seqs.fa.

# 0-based
seqtool trim -0 ' 2..6' seqs.fa
```

#### Empty ranges

Note that ranges of zero length are only possible if
the start is greater than the end, e.g.: `5..4`.
