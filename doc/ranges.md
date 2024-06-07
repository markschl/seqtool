# Explanation of ranges

Ranges in seqtool are used or produced by commands like [trim](trim), [find](find),
[mask](mask), and [slice](slice).

## In a nutshell

1. Ranges in the form `start:end` include both the start and end position,
   unless [0-based coordinates](#0-based-coordinates--0) are configured.
2. Negative coordinates (e.g. `-5:-1`) indicate coordinate offsets from the end
3. [Unbounded](#unbounded-ranges-start-or-end) ranges (`start:` or `:end`)
   have only one defined coordinate, while the range includes everything on the
   other side. *"undefined"* equals to missing coordinates.
4. If interpreting ranges as [exclusive](#exclusive-ranges--e--exclusive), the
   actual start or end positions are not included in the range.

## Overview

Ranges look like this: `start:end`.
The the start and end positions are **always part of the range**, unless
explicitly switching to [0-based coordinates](#0-based-coordinates--0).

It is also possible to use negative numbers: `-1` references the last character
in the sequence, `-2` the second last, and so on.

<pre>
                   <span style="color:blue">| <—————————————> | </span>
sequence:       A  <span style="color:blue"> T   G   C   A   T</span>   G   C
base number:    1  <span style="color:blue"> 2   3   4   5   6</span>   7   8
from end:      -8  <span style="color:blue">-7  -6  -5  -4  -3</span>  -2  -1
</pre>

The following commands all trim sequences to the <span style="color:blue">blue</span> range,
resulting in the same output:

```sh
st trim '2:6' input.fasta
st trim '-7:-3' input.fasta
st trim '2:-3' input.fasta
```

## Empty ranges

Ranges of zero length are only possible if the start is greater than the end
(e.g. `5:4`).
*seqtool* interprets all ranges where *start > end* as
empty.

An exception are [0-based ranges](#0-based-coordinates--0).
In this specific mode, `5:5` would result in an empty range.


## Unbounded ranges: `start:` or `:end`

The start or end positions can be missing, which results in the whole sequence
up or from a certain position being included in the range.

### No end

The following retains all positions from `5` to the end:

```sh
st trim '5:' input.fasta
st trim '-4:' input.fasta
```

<pre>
                               <span style="color:blue">| <——————————></span>
sequence:       A   T   G   C  <span style="color:blue"> A   T   G   C </span>
base number:    1   2   3   4  <span style="color:blue"> 5   6   7   8 </span>
from end:      -8  -7  -6  -5  <span style="color:blue">-4  -3  -2  -1 </span>
</pre>

The sequence ends at position 8, so `5:` is equivalent to `5:8` or `5:-1`.

However, if sequence lengths differ, only `5:` or `5:-1` will include *everything*
after position 5, while `5:8` would still only return these fixed positions:

<pre>
ATGC<span style="color:blue">ATGC</span>
ATGC<span style="color:blue">ATGCMORE</span>
</pre>

> ⚠️ `5:` is equivalent to `5:-1` here, but results can differ with
> [exclusive ranges](#exclusive-ranges--e--exclusive).
Usually, you might want to use the unbounded `start:` range, which will *always*
include the whole sequence end.

### No start

It is also possible to omit the **start** position to return all positions up to
a given position:

```sh
st trim ':3' input.fasta
```

<pre>
<span style="color:blue">ATG</span>CATGC
<span style="color:blue">ATG</span>CATGCMORE
</pre>

> ⚠️ again, `0:3` is equivalent to `:3`, but only if not using
> [exclusive ranges](#exclusive-ranges--e--exclusive).

### No bounds at all

The following will retain the whole sequence, resulting in *no trimming* at all:

```sh
st trim ":" input.fasta
```

<pre>
<span style="color:blue">ATGCATGC
ATGCATGCMORE</span>
</pre>

### `undefined`

*Undefined* is a special keyword that equals to missing data and thus,
`undefined:undefined` equals to an unbounded range `:`.

`undefined` may be returned by functions such as `opt_attr()` and `opt_meta()`.

## Exclusive ranges (`-e/--exclusive`)

The `trim` and `mask` commands also accept an `-e/--exclusive` argument
that excludes start and end coordinates from the range.

The following commands trim to positions 3-5 (<span style="color:blue">blue</span>)
without the range bounds `2` and `6` themselves (<span style="color:red">red</span>).

```sh
st trim -e '2:6' input.fasta
st trim -e '-7:-3' input.fasta
```

<pre>
                       <span style="color:blue">| <——————> |</span>
sequence:       A <span style="color:red">  T </span><span style="color:blue">  G   C   A </span><span style="color:red">  T</span>   G   C
base number:    1 <span style="color:red">  2 </span><span style="color:blue">  3   4   5 </span><span style="color:red">  6</span>   7   8
from end:      -8 <span style="color:red"> -7 </span><span style="color:blue"> -6  -5  -4 </span><span style="color:red"> -3</span>  -2  -1
</pre>

One important corner case are **[unbounded ranges](#unbounded-ranges-start-or-end)**.
In case of missing bounds, the ranges are not trimmed or masked on that side, the range
still extends to the start or end as if it would without `-e/--exclusive`:


```sh
st trim -e '5:' input.fasta
st trim -e '-4:' input.fasta
```

<pre>
                                   <span style="color:blue">| <——————></span>
sequence:       A   T   G   C  <span style="color:red"> A </span><span style="color:blue">  T   G   C </span>
base number:    1   2   3   4  <span style="color:red"> 5 </span><span style="color:blue">  6   7   8 </span>
from end:      -8  -7  -6  -5  <span style="color:red">-4 </span><span style="color:blue"> -3  -2  -1 </span>
</pre>


## 0-based coordinates (`-0`)

If you prefer 0-based ranges common to many programming languages, specify the `-0` argument.
These are less intuitive, but have the advantage that empty slices can be more easily
obtained (e.g. `st trim -0 1:1`).

The range indices start with `0` instead of `1`, and the range end (<span style="color:green">green</span>)
is *not included* in the slice. Negative indices are also possible and work exactly as in Python.

<pre>
                   <span style="color:blue">| <—————————————> | </span>
sequence:       A  <span style="color:blue"> T   G   C   A   T</span>   G   C
base number:    1  <span style="color:blue"> 2   3   4   5   6</span>   7   8
0-based start:  0  <span style="color:blue"> 1   2   3   4   5</span>  <span style="color:green"> 6</span>   7
from end:      -8  <span style="color:blue">-7  -6  -5  -4  -3</span>  <span style="color:green">-2</span>  -1
</pre>

```sh
st trim -0 '1:6' input.fasta
st trim -0 '-7:-2' input.fasta
```
