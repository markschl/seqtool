This command allows for viewing sequences in the terminal. The output
is colored if the terminal supports colors. On UNIX systems (Linux, Mac OS, ...),
the sequences are directly forwarded to the `less` pager command, which allows for
navigating up and down or in horizontal direction. On Windows, this is not done.

The first sequence line in the input is always used to determine the
sequence type (DNA/RNA or Protein).


Example view of DNA sequences:

```sh
st view seqs.fasta
```

![DNA sequence](img/base_view.png)


```sh
st view H1.fasta
```

View of [Histone H1 sequences](https://www.ncbi.nlm.nih.gov/research/HistoneDB2.0/index.fcgi/type/H1/#msa_div_browse),
colored according to the [RasMol scheme](http://www.openrasmol.org/doc/#aminocolours).

![Histone H1](img/h1.png)

If quality scores are present (from FASTQ or QUAL files), the background is colored
accordingly (configure with `--qscale` and `--qmax`):

```sh
st view seqs.fastq
```

![Sequence quality](img/qual_view.png)


## Palettes

There are multiple color schemes/palettes available, which can be configured
using `--dna-pal`, `--aa-pal` and `--qscale`.

A visualization of the builtin palettes is obtained with `st view --list-pal`:

![Palettes](img/palettes.png)
