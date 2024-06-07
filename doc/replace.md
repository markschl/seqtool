### RNA to DNA

Simple RNA to DNA conversion by replacing all occurrences of `U` with `T`:

```sh
st replace U T rna.fasta > dna.fasta
```

### Regular expressions

The following command extracts Genbank accessions from sequence headers that follow
the old-style Genbank format, e.g. `gi|1031916024|gb|KU317675.1|`:

```sh
st replace -ir "gi\|\d+\|[a-z]+\|(?<acc>.+?)\|.*" seqs.fasta '$acc' > seqs_accession.fasta
```

The accession `KU317675` is matched by the named regex group 'acc' and
referenced by `'$acc'`.
Regular expression (regex) groups can be accessed with
[the '$' prefix](https://docs.rs/regex/latest/regex/#example-replacement-with-named-capture-groups)
in replacements.

> You can use online tools such as https://regex101.com to build and debug your
> regular expression

> ⚠ In Bash, make sure to use single quotes around `'$acc'` (not double quotes) to avoid
> that `$acc` is interpreted as a shell variable.

