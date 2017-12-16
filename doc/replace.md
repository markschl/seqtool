Example: Conversion of RNA to DNA by replacing all occurrences of `U` with `T`:

```bash
seqtool replace rna.fa U T > dna.fa
```

Regular expression (regex) groups can be accessed with
[the '$' prefix](https://doc.rust-lang.org/regex/regex/index.html#example-replacement-with-named-capture-groups)
in replacements.
