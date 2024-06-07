# Metadata from delimited files

It often occurs that information from text files (created manually or
using another program) needs to be integrated.
This can be done in every *seqtool* command with the `-m/--meta` option.
The delimiter and the column containing the sequence IDs can be configured
with `--meta-delim` and `--id-col`.

Fields are accessible as [variables](variables) in this form: `meta(column)` where
*column* is either a number or the header name of the given column.

Consider this list containing taxonomic information about sequences (*genus.tsv*):

```
id  genus
seq1  Actinomyces
seq2  Amycolatopsis
(...)
```

The genus name can be added to the FASTA header using this command:

```sh
st set --meta genus.tsv --desc '{meta(genus)}' input.fasta > with_genus.fasta
# short:
st set -m genus.tsv -d '{meta(genus)}' input.fasta > with_genus.fasta
```

```
>seq1 Actinomyces
SEQUENCE
>seq2 Amycolatopsis
SEQUENCE
(...)
```

If any of the sequence IDs is not found in the metadata, there will be an error.
If missing data is expected, use `opt_meta` instead.
Missing entries are `undefined`:

```sh
st set -m genus.tsv --desc '{opt_meta(genus)}' input.fasta > with_genus.fasta
```

```
>seq1 Actinomyces
SEQUENCE
>seq2 Amycolatopsis
SEQUENCE
>seq3 undefined
SEQUENCE
(...)
```

### Filtering by ID

Sometimes it is necessary to select all sequence records present in a list of
sequence IDs. This can easily be achieved using this command:

```sh
st filter -m id_list.txt 'has_meta()' seqs.fasta > in_list.fasta
```

### Multiple metadata sources

Several sources can be simultaneously used in the same command with
`-m file1 -m file2 -m file3...`:

```sh
st filter -m source1.txt -m source2.txt 'meta("column", 1) == "value" && has_meta(2)' seqs.fasta > in_list.fasta
```

> Sources are referenced using `meta(column, file_number)` or `has_meta(file_number)`

### All variables

(see [variable reference](var_reference/#access-metadata-from-delimited-text-files)).
