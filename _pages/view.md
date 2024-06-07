View biological sequences, colored by base / amino acid, or by sequence quality

```
Usage: st view [OPTIONS] [INPUT]...

Options:
  -h, --help  Print help

General 'view' command options:
  -i, --id-len <CHARS>  Length of IDs in characters. Longer IDs are truncated
                        (default: 10 - 100 depending on ID length)
  -d, --show-desc       Show descriptions along IDs if there is enough space
      --fg              Color base / amino acid letters instead of background.
                        If base qualities are present, background coloration is
                        shown, and the foreground scheme will be 'dna-bright'
                        (change with --dna-pal)
  -n, --n-max <N>       View only the top <N> sequences without pager. Automatic
                        handoff to a pager is only available in UNIX (turn off
                        with --no-pager) [default: 100]

View pager (UNIX only):
      --no-pager       Disable paged display
      --pager <PAGER>  Pager command to use [env: ST_PAGER=] [default: "less
                       -RS"]
  -b, --break          Break lines in pager, disabling 'horizontal scrolling'.
                       Equivalent to --pager 'less -R'

Colors:
      --list-pal           Show a list of all builtin palettes and exit
      --dna-pal <PAL>      Color mapping for DNA. Palette name (hex code,
                           CSS/SVG color name) or list of
                           'base1:rrggbb,base2:rrggbb,...' (builtin palettes:
                           dna, dna-bright, dna-dark, pur-pyrimid, gc-at)
                           [default: dna]
      --aa-pal <PAL>       Color mapping for amino acids. Palette name (hex
                           code, CSS/SVG color name) or list of
                           'base1:rrggbb,base2:rrggbb,...' (available: rasmol,
                           polarity) [default: rasmol]
      --qscale <PAL>       Color scale to use for coloring according to base
                           quality. Palette name (hex code, CSS/SVG color name)
                           or list of 'base1:rrggbb,base2:rrggbb,...' Palette
                           name or sequence of hex codes from low to high
                           [default: red-blue]
      --textcols <COLORS>  Text colors used with background coloring. Specify
                           as: <dark>,<bright>. Which one is used will be chosen
                           depending on the brightness of the background
                           [default: 333333,eeeeee]
  -t, --truecolor <?>      Use 16M colors, not only 256. This has to be
                           supported by the terminal. Useful if autorecognition
                           fails [possible values: true, false]
```

[See this page](opts) for the options common to all commands.

## Details
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

![DNA sequence](/assets/images/base_view.png)


```sh
st view H1.fasta
```

View of [Histone H1 sequences](https://www.ncbi.nlm.nih.gov/research/HistoneDB2.0/index.fcgi/type/H1/#msa_div_browse),
colored according to the [RasMol scheme](http://www.openrasmol.org/doc/#aminocolours).

![Histone H1](/assets/images/h1.png)

If quality scores are present (from FASTQ or QUAL files), the background is colored
accordingly (configure with `--qscale` and `--qmax`):

```sh
st view seqs.fastq
```

![Sequence quality](/assets/images/qual_view.png)


## Palettes

There are multiple color schemes/palettes available, which can be configured
using `--dna-pal`, `--aa-pal` and `--qscale`.

A visualization of the builtin palettes is obtained with `st view --list-pal`:

![Palettes](/assets/images/palettes.png)
