use super::*;

static ATTR_FA: &str = ">seq;a=0 b=3\nATGC\n";

#[test]
fn general() {
    let fa = ">seq1\nSEq\n>seq2\nsEq\n>seq3\nSEQ\n";
    let t = Tester::new();
    t.cmp(
        &[".", "--to-tsv", "id,seq_num,seq_idx,upper_seq,lower_seq"],
        fa,
        "seq1\t1\t0\tSEQ\tseq\nseq2\t2\t1\tSEQ\tseq\nseq3\t3\t2\tSEQ\tseq\n",
    );
}

#[test]
#[cfg(any(feature = "all-commands", feature = "count"))]
fn numeric() {
    let t = Tester::new();
    t.cmp(&["count", "-k", "num('1.2')"], *FASTA, "1.2\t4\n")
        .cmp(
            &["count", "-k", "bin(1.1, .1)"],
            *FASTA,
            "(1.1, 1.20000]\t4\n",
        );

    #[cfg(feature = "expr")]
    t.cmp(&["count", "-k", "{ num(2 + 1) }"], *FASTA, "3\t4\n")
        .cmp(
            &["count", "-k", "{ num(attr('p') + 1) }"],
            *FASTA,
            "11\t1\n21\t1\n101\t1\n111\t1\n",
        )
        .cmp(
            &["count", "-k", "{ bin(attr('p') > 2 ? 2 : attr('p')) }"],
            *FASTA,
            "(1, 2]\t1\n(2, 3]\t3\n",
        )
        .fails(
            &["count", "-k", "{ num('abc') + 1 }"],
            *FASTA,
            "Could not convert 'abc'",
        )
        .fails(
            &["count", "-k", "{ num('abc' + 1) }"],
            *FASTA,
            "Could not convert 'abc1'",
        );
}

#[test]
fn attrs() {
    let t = Tester::new();
    t.cmp(&[".", "--to-tsv", "attr(p)"], *FASTA, "2\n1\n10\n11\n")
        .cmp(&[".", "--to-tsv", "attr(b)"], ATTR_FA, "3\n")
        .cmp(&[".", "--to-tsv", "has_attr(b)"], ATTR_FA, "true\n")
        .cmp(
            &[".", "--to-tsv", r"{has_attr('x\'y')}"],
            ">id x'y=0\nSEQ\n",
            "true\n",
        )
        .cmp(
            &[".", "-a", "c={attr(a)}", "-a", "b={attr(a)}"],
            ">ID a=0 b=1 c=2\nSEQ",
            ">ID a=0 b=0 c=0\nSEQ\n",
        )
        .fails(
            &[".", "--to-tsv", "id", "-a", "a=0"],
            *FASTA,
            "output format is not FASTA or FASTQ",
        )
        .fails(
            &[".", "-a", "a=0", "-a", "a=1"],
            *FASTA,
            "attribute 'a' is added/edited twice",
        )
        .fails(
            &[".", "-A", "a=0", "-a", "a=1"],
            *FASTA,
            "attribute 'a' is supposed to be appended",
        )
        .fails(
            &[".", "-A", "p={attr(p)}"],
            *FASTA,
            "attribute 'p' is supposed to be appended",
        )
        .fails(
            &[".", "-a", "a={attr_del(a)}"],
            *FASTA,
            "attribute 'a' is supposed to be deleted",
        )
        .fails(
            &[".", "-a", "a={attr_del(p)}_{attr(p)}"],
            *FASTA,
            "attribute 'p' is supposed to be deleted",
        );

    #[cfg(feature = "expr")]
    t.cmp(
        &[".", "--to-tsv", "{num(attr('p'))+1}"],
        *FASTA,
        "3\n2\n11\n12\n",
    )
    // edit using the earlier value of itself
    .cmp(
        &[".", "-a", "b={attr('b')*3}"],
        ATTR_FA,
        ">seq;a=0 b=9\nATGC\n",
    );
}

#[test]
fn attrs_missing() {
    let t = Tester::new();
    t.cmp(&[".", "--to-tsv", "opt_attr(a)"], ATTR_FA, "\n")
        .cmp(&[".", "--to-tsv", "has_attr(a)"], ATTR_FA, "false\n")
        .fails(
            &[".", "--to-tsv", "attr(a)"],
            ATTR_FA,
            "not found in record",
        );
    #[cfg(feature = "expr")]
    t.cmp(
        &[".", "--to-tsv", "{opt_attr('a') === undefined}"],
        ATTR_FA,
        "true\n",
    );
}

#[test]
fn attr_format() {
    let fa = ">seq;a=0 |b__3|c:2 |d:  5\nATGC\n";
    let t = Tester::new();
    t.cmp(
        &[".", "--to-tsv", "attr(a)", "--attr-fmt", ";key=value"],
        fa,
        "0\n",
    )
    .cmp(
        &[".", "--to-tsv", "has_attr(a)", "--attr-fmt", ";key=value"],
        fa,
        "true\n",
    )
    .cmp(&[".", "--to-tsv", "opt_attr(a)"], fa, "\n")
    .fails(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", ";key=value"],
        fa,
        "not found in record",
    )
    .cmp(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key__value"],
        fa,
        "3\n",
    )
    .cmp(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key_value"],
        fa,
        "_3\n",
    )
    .fails(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key___value"],
        fa,
        "not found in record",
    )
    .cmp(
        &[".", "--to-tsv", "attr(c)", "--attr-fmt", "|key:value"],
        fa,
        "2 \n",
    );
    // with env vars
    let mut t = Tester::new();
    t.env("ST_ATTR_FORMAT", ";key=value")
        .cmp(&[".", "--to-tsv", "attr(a)"], fa, "0\n");
    t.env("ST_ATTR_FORMAT", "|key:value")
        .cmp(&[".", "--to-tsv", "attr(c)"], fa, "2 \n")
        .cmp(&[".", "--to-tsv", "attr(d)"], fa, "  5\n");
}

#[test]
fn attr_set() {
    let t = Tester::new();
    t.cmp(
        &[".", "--to-tsv", "id,desc,seq,attr(b),{attr(b)}"],
        ATTR_FA,
        "seq;a=0\tb=3\tATGC\t3\t3\n",
    )
    .cmp(
        &[".", "-a", "b={attr(a)}", "--attr-fmt", ";key=value"],
        ATTR_FA,
        ">seq;a=0;b=0 b=3\nATGC\n",
    )
    .cmp(
        &[".", "-a", "c={attr(b)}"],
        ATTR_FA,
        ">seq;a=0 b=3 c=3\nATGC\n",
    )
    .cmp(
        &[".", "-a", "c={attr_del(b)}"],
        ATTR_FA,
        ">seq;a=0 c=3\nATGC\n",
    );
}

static META: &str = "
seq1\t2
seq0\t1
seq3\t10
seq2\t11";

#[test]
fn meta() {
    let t: Tester = Tester::new();
    // Contains the p=... attribute values of the FASTA records
    t.temp_file("meta", Some(META), |p, _| {
        let exp = "2\n1\n10\n11\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, exp);
        #[cfg(feature = "expr")]
        t.cmp(
            &[".", "-m", p, "--to-tsv", "{ attr('p') - meta(2) }"],
            *FASTA,
            "0\n0\n0\n0\n",
        );
        // invalid column
        let msg = "column numbers must be > 0";
        t.fails(&[".", "-m", p, "--to-tsv", "{meta(0)}"], *FASTA, msg);
        let msg = "Column no. 3 not found in metadata entry for record 'seq1'";
        t.fails(&[".", "-m", p, "--to-tsv", "{meta(3)}"], *FASTA, msg);
    });
}

#[test]
fn meta_delim() {
    let t: Tester = Tester::new();
    let input = META.replace('\t', ",");
    t.temp_file("meta", Some(&input), |p, _| {
        let exp = "seq1,2\nseq0,1\nseq3,10\nseq2,11\n";
        t.cmp(
            &[".", "-m", p, "--meta-delim", ",", "--to-csv", "id,meta(2)"],
            *FASTA,
            exp,
        );
    });
}

static META_HEADER: &str = "
id\tnumber
seq1\t2
seq0\t1
seq3\t10
seq2\t11";

#[test]
fn meta_header() {
    let t: Tester = Tester::new();
    t.temp_file("meta", Some(META_HEADER), |p, _| {
        // header is ignored (ID not matching)
        let out = "2\n1\n10\n11\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, out);
        // activate auto-header
        let out = "2\t2\n1\t1\n10\t10\n11\t11\n";
        t.cmp(
            &[".", "-m", p, "--to-tsv", "{meta(2)},{meta(number)}"],
            *FASTA,
            out,
        );
        let msg = "Column 'somecol' not found";
        t.fails(&[".", "-m", p, "--to-tsv", "{meta(somecol)}"], *FASTA, msg);
    });
}

// partially ordered
static META_UNORDERED: &str = "
seq1\t2
seq0\t1
seq2\t11
seq3\t10";

// totally unordered
static META_UNORDERED2: &str = "
seq3\t10
seq1\t2
seq0\t1
seq2\t11";

#[test]
fn meta_unordered() {
    let t = Tester::new();
    t.temp_file("meta", Some(META_UNORDERED), |p, _| {
        let out = "2\n1\n10\n11\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, out);
    });
    t.temp_file("meta", Some(META_UNORDERED2), |p, _| {
        let out = "2\n1\n10\n11\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, out);
    });
}

static META_MISSING: &str = "
seq3\t10
seq1\t2
seq2\t11";

#[test]
fn meta_missing() {
    let t = Tester::new();
    t.temp_file("meta", Some(META_MISSING), |p, _| {
        // opt_meta
        let out = "2\n\n10\n11\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{opt_meta(2)}"], *FASTA, out);
        // has_meta
        let out = "true\nfalse\ntrue\ntrue\n";
        t.cmp(&[".", "-m", p, "--to-tsv", "{has_meta}"], *FASTA, out);
        // meta() should fail
        let msg = "not found in metadata";
        t.fails(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, msg);
    });
}

static META_DUPLICATED: &str = "
seq3\t10
seq1\t2
seq0\t1
seq1\t2
seq0\t1
seq2\t11";

#[test]
fn meta_duplicated_entries() {
    let t = Tester::new();
    t.temp_file("meta", Some(META_DUPLICATED), |p, _| {
        let out = "2\n1\n10\n11\n";
        t.cmp(
            &[".", "-m", p, "--to-tsv", "{meta(2)}", "--dup-ids"],
            *FASTA,
            out,
        );
        let msg = "Found duplicate IDs in associated metadata (first: seq1)";
        t.cmp_stderr(&[".", "-m", p, "--to-tsv", "{meta(2)}"], *FASTA, out, msg);
    });
}

#[test]
fn meta_multi_file() {
    let t = Tester::new();
    t.temp_file("meta", Some(META_UNORDERED), |f1, _| {
        t.temp_file("meta", Some(META_HEADER), |f2, _| {
            t.temp_file("meta", Some(META_MISSING), |f3, _| {
                // three files
                let fields = "meta(2, 1),meta(2, 2),opt_meta(2, 3)";
                let out = "2\t2\t2\n1\t1\t\n10\t10\t10\n11\t11\t11\n";
                t.cmp(
                    &[".", "-m", f1, "-m", f2, "-m", f3, "--to-tsv", fields],
                    *FASTA,
                    out,
                );
                let fields = "meta(2, 1),meta(2, 2),meta(2, 3)";
                let msg = "not found in metadata";
                t.fails(
                    &[".", "-m", f1, "-m", f2, "-m", f3, "--to-tsv", fields],
                    *FASTA,
                    msg,
                );
                // invalid index
                let msg = "Invalid metadata file no. requested: 0";
                t.fails(&[".", "-m", f1, "--to-tsv", "has_meta(0)"], *FASTA, msg);
                let msg = "Metadata file no. 2 was requested";
                t.fails(&[".", "-m", f1, "--to-tsv", "meta(1, 2)"], *FASTA, msg);
            });
        });
    });
}

/// Test missing and duplicated IDs with larger input
#[test]
fn meta_larger() {
    // For IDs 1-4, sequences and metadata are in same order -> 'in-sync' mode,
    // duplicates not detected.
    // 7 breaks the order -> switch to hash map index
    // From the remaining, 2 and 5 have no metadata entry in the HashMap
    //   (5 totally missing, 2 was read in 'sync mode')
    let ids = [1, 2, 2, 3, 4, 7, 2, 5, 6];
    let _meta = [(1, 1), (2, 2), (2, 0), (3, 3), (4, 4), (6, 6), (7, 7)];
    // expected output
    let _out = [
        "1\t1", "2\t2", "2\t0", "3\t3", "4\t4", "7\t7", "2\t", "5\t", "6\t6",
    ];
    // with --dup-ids: always the same value for 2
    let _dup = [
        "1\t1", "2\t2", "2\t2", "3\t3", "4\t4", "7\t7", "2\t2", "5\t", "6\t6",
    ];
    let fasta = ids.iter().map(|i| format!(">{}\nSEQ\n", i)).join("");
    let meta = _meta
        .iter()
        .map(|(i, m)| format!("{}\t{}\n", i, m))
        .join("");
    let out = _out.iter().map(|i| format!("{}\n", i)).join("");
    let idx_out = _dup.iter().map(|i| format!("{}\n", i)).join("");

    let t = Tester::new();
    let fields = "id,opt_meta(2)";
    let dup_m = "Found duplicate IDs in associated metadata (first: 2)";
    t.temp_file("meta", Some(&meta), |path, _| {
        // 'hybrid' in-sync and hash map index parsing: duplication of no. 2 is not recognized
        t.cmp(&[".", "-m", path, "--to-tsv", fields], &fasta, &out);
        // using hash map index from start: duplicate entry is recognized
        t.cmp_stderr(
            &[".", "-m", path, "--to-tsv", fields, "--dup-ids"],
            &fasta,
            &idx_out,
            dup_m,
        );
        // 'meta' fails because of missing data
        let msg = "ID '2' not found in metadata";
        t.fails(&[".", "-m", path, "--to-tsv", "id,meta(2)"], &fasta, msg);
        // adding another sequence with ID=5 fails without --dup-ids
        // *note*: adding ID=7 would not fail, since the entry that breaks the
        // 'sync reading' is not checked for duplication.
        let fasta2 = format!("{}>5\nSEQ\n", fasta);
        let dup_err = "Found duplicate sequence ID: '5'.";
        t.fails(&[".", "-m", path, "--to-tsv", fields], &fasta2, dup_err);
        let exp = format!("{}5\t\n", idx_out);
        t.cmp(
            &[".", "-m", path, "--to-tsv", fields, "--dup-ids"],
            &fasta2,
            &exp,
        );
    });
    // adding a duplicate metadata entry with ID=7 yields a message
    // even without --dup-ids
    let meta2 = format!("{}7\t700\n", meta);
    t.temp_file("meta", Some(&meta2), |path, _| {
        // in this case, the duplicate ID = 7 is recognized even without --dup-ids
        // (previous entry with ID = 7 was read while in 'hash map mode')
        let dup_msg = "Found duplicate IDs in associated metadata (first: 7).";
        t.cmp_stderr(
            &[".", "-m", path, "--to-tsv", fields],
            &fasta,
            &out,
            dup_msg,
        );
    });
}

#[test]
#[cfg(feature = "gz")]
fn meta_compressed() {
    // first, compress some metadata
    let t = Tester::new();
    t.temp_file("compr_meta.csv.gz", None, |path, _| {
        t.succeeds(&[".", "--outfields", "id,attr(p)", "-o", path], *FASTA);
        let exp = "seq1,2\nseq0,1\nseq3,10\nseq2,11\n";
        t.cmp(
            &[
                ".",
                "-m",
                path,
                "--meta-delim",
                ",",
                "--to-csv",
                "id,meta(2)",
            ],
            *FASTA,
            exp,
        );
    });
}

// expressions: regexes with variables inside will yield errors
// test quoted stuff
// charcount(GC) / seqlen == gc / 100
// --to-tsv id,{charcount('GC')/seqlen}
// --to-tsv id,{charcount("GC,")/seqlen}
// --to-tsv id,,...
// MDN: const re = /\w+/;
// OR
// const re = new RegExp("\\w+");
