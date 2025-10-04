use crate::helpers::NA;

use super::*;

static ATTR_FA: &str = ">seq;a=0 b=3\nATGC\n";

#[test]
fn general() {
    with_tmpdir("st_vars_general_", |td| {
        let fa = concat!(">seq1\nSEq\n", ">seq2\nsEq\n", ">seq3\nSEQ\n");
        let v = "id,seq_num,seq_idx,upper_seq,lower_seq";
        let exp = concat!(
            "seq1,1,0,SEQ,seq\n",
            "seq2,2,1,SEQ,seq\n",
            "seq3,3,2,SEQ,seq\n"
        );
        cmp(&[".", "--to-csv", v], fa, exp);
        // seq_idx / seq_num
        let v = "id,seq_num,seq_num(true),seq_idx,seq_idx(true)";
        let input = td.multi_file(".fasta", [fa, fa]);
        let exp = concat!(
            "seq1,1,1,0,0\n",
            "seq2,2,2,1,1\n",
            "seq3,3,3,2,2\n",
            "seq1,4,1,3,0\n",
            "seq2,5,2,4,1\n",
            "seq3,6,3,5,2\n"
        );
        cmp(&[".", "--to-csv", v], input, exp);
    });
}

#[test]
#[cfg(any(feature = "all-commands", feature = "count"))]
fn numeric() {
    cmp(&["count", "-k", "num('1.2')"], &*FASTA, "1.2\t4\n");
    cmp(
        &["count", "-k", "bin(1.1, .1)"],
        &*FASTA,
        "(1.1, 1.20000]\t4\n",
    );

    #[cfg(feature = "expr")]
    {
        cmp(&["count", "-k", "{ num(2 + 1) }"], &*FASTA, "3\t4\n");
        cmp(
            &["count", "-k", "{ num(attr('p') + 1) }"],
            &*FASTA,
            "11\t1\n21\t1\n101\t1\n111\t1\n",
        );
        cmp(
            &["count", "-k", "{ bin(attr('p') > 2 ? 2 : attr('p')) }"],
            &*FASTA,
            "(1, 2]\t1\n(2, 3]\t3\n",
        );
        fails(
            &["count", "-k", "{ num('abc') + 1 }"],
            &*FASTA,
            "Could not convert 'abc'",
        );
        fails(
            &["count", "-k", "{ num('abc' + 1) }"],
            &*FASTA,
            "Could not convert 'abc1'",
        );
    }
}

#[test]
fn attrs() {
    cmp(&[".", "--to-tsv", "attr(p)"], &*FASTA, "2\n1\n10\n11\n");
    cmp(&[".", "--to-tsv", "attr(b)"], ATTR_FA, "3\n");
    cmp(&[".", "--to-tsv", "has_attr(b)"], ATTR_FA, "true\n");
    cmp(
        &[".", "--to-tsv", r"{has_attr('x\'y')}"],
        ">id x'y=0\nSEQ\n",
        "true\n",
    );
    cmp(
        &[".", "-a", "c={attr(a)}", "-a", "b={attr(a)}"],
        ">ID a=0 b=1 c=2\nSEQ",
        ">ID a=0 b=0 c=0\nSEQ\n",
    );
    fails(
        &[".", "-a", "a=0", "-a", "a=1"],
        &*FASTA,
        "attribute 'a' is added/edited twice",
    );
    fails(
        &[".", "-A", "a=0", "-a", "a=1"],
        &*FASTA,
        "the 'a' attribute is also used in a different way",
    );
    fails(
        &[".", "-A", "p={attr(p)}"],
        &*FASTA,
        "the 'p' attribute is also used in a different way",
    );
    fails(
        &[".", "-a", "a={attr_del(a)}"],
        &*FASTA,
        "attribute 'a' is supposed to be deleted",
    );
    fails(
        &[".", "-a", "a={attr_del(p)}_{attr(p)}"],
        &*FASTA,
        "attribute 'p' is supposed to be deleted",
    );
    // undefined
    cmp(
        &[".", "-a", "b={opt_attr('a')}"],
        format!(">seq a={NA}\nSEQ\n"),
        &format!(">seq a={NA} b={NA}\nSEQ\n"),
    );
    cmp(
        &[".", "-a", "b={has_attr('a')}"],
        format!(">seq a={NA}\nSEQ\n"),
        &format!(">seq a={NA} b=false\nSEQ\n"),
    );
    fails(
        &[".", "-a", "b={attr('a')}"],
        format!(">seq a={NA}\nSEQ\n"),
        &format!("value for attribute 'a' is '{NA}', which is reserved for missing values"),
    );

    #[cfg(feature = "expr")]
    {
        cmp(
            &[".", "--to-tsv", "{num(attr('p'))+1}"],
            &*FASTA,
            "3\n2\n11\n12\n",
        );
        // edit using the earlier value of itself
        cmp(
            &[".", "-a", "b={attr('b')*3}"],
            ATTR_FA,
            ">seq;a=0 b=9\nATGC\n",
        );
    }
}

#[test]
fn attrs_missing() {
    cmp(
        &[".", "--to-tsv", "opt_attr(a)"],
        ATTR_FA,
        &format!("{NA}\n"),
    );
    cmp(&[".", "--to-tsv", "has_attr(a)"], ATTR_FA, "false\n");
    fails(
        &[".", "--to-tsv", "attr(a)"],
        ATTR_FA,
        "not found in record",
    );
    #[cfg(feature = "expr")]
    cmp(
        &[".", "--to-tsv", "{opt_attr('a') === undefined}"],
        ATTR_FA,
        "true\n",
    );
}

#[test]
fn attr_format() {
    let fa = ">seq;a=0 |b__3|c:2 |d:  5\nATGC\n";

    cmp(
        &[".", "--to-tsv", "attr(a)", "--attr-fmt", ";key=value"],
        fa,
        "0\n",
    );
    cmp(
        &[".", "--to-tsv", "has_attr(a)", "--attr-fmt", ";key=value"],
        fa,
        "true\n",
    );
    cmp(&[".", "--to-tsv", "opt_attr(a)"], fa, &format!("{NA}\n"));
    fails(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", ";key=value"],
        fa,
        "not found in record",
    );
    cmp(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key__value"],
        fa,
        "3\n",
    );
    cmp(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key_value"],
        fa,
        "_3\n",
    );
    fails(
        &[".", "--to-tsv", "attr(b)", "--attr-fmt", "|key___value"],
        fa,
        "not found in record",
    );
    cmp(
        &[".", "--to-tsv", "attr(c)", "--attr-fmt", "|key:value"],
        fa,
        "2 \n",
    );
    // with env vars
    let e = [("ST_ATTR_FORMAT", ";key=value")];
    cmp_with_env(&[".", "--to-tsv", "attr(a)"], fa, "0\n", e);
    let e = [("ST_ATTR_FORMAT", "|key:value")];
    cmp_with_env(&[".", "--to-tsv", "attr(c)"], fa, "2 \n", e);
    cmp_with_env(&[".", "--to-tsv", "attr(d)"], fa, "  5\n", e);
}

#[test]
fn attr_set() {
    cmp(
        &[".", "--to-tsv", "id,desc,seq,attr(b),{attr(b)}"],
        ATTR_FA,
        "seq;a=0\tb=3\tATGC\t3\t3\n",
    );
    cmp(
        &[".", "-a", "b={attr(a)}", "--attr-fmt", ";key=value"],
        ATTR_FA,
        ">seq;a=0;b=0 b=3\nATGC\n",
    );
    cmp(
        &[".", "-a", "c={attr(b)}"],
        ATTR_FA,
        ">seq;a=0 b=3 c=3\nATGC\n",
    );
    cmp(
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
    // Contains the p=... attribute values of the FASTA records
    let meta = tmp_file("st_meta", ".tsv", META);
    let exp = "2\n1\n10\n11\n";
    cmp(&[".", "-m", &meta, "--to-tsv", "{meta(2)}"], &*FASTA, exp);
    #[cfg(feature = "expr")]
    cmp(
        &[".", "-m", &meta, "--to-tsv", "{ attr('p') - meta(2) }"],
        &*FASTA,
        "0\n0\n0\n0\n",
    );
    // invalid column
    let msg = "column numbers must be > 0";
    fails(&[".", "-m", &meta, "--to-tsv", "{meta(0)}"], &*FASTA, msg);
    let msg = "Column no. 3 not found in metadata entry for record 'seq1'";
    fails(&[".", "-m", &meta, "--to-tsv", "{meta(3)}"], &*FASTA, msg);
}

#[test]
fn meta_delim() {
    let meta = tmp_file("st_meta_delim_", ".tsv", &META.replace('\t', ","));

    let exp = "seq1,2\nseq0,1\nseq3,10\nseq2,11\n";
    cmp(
        &[
            ".",
            "-m",
            &meta,
            "--meta-delim",
            ",",
            "--to-csv",
            "id,meta(2)",
        ],
        &*FASTA,
        exp,
    );
}

static META_HEADER: &str = "
id\tnumber
seq1\t2
seq0\t1
seq3\t10
seq2\t11";

#[test]
fn meta_header() {
    let meta = tmp_file("st_meta_head_", ".tsv", META_HEADER);
    // header is ignored (ID not matching);
    let out = "2\n1\n10\n11\n";
    cmp(&[".", "-m", &meta, "--to-tsv", "{meta(2)}"], &*FASTA, out);
    // activate auto-headermeta
    let out = "2\t2\n1\t1\n10\t10\n11\t11\n";
    cmp(
        &[".", "-m", &meta, "--to-tsv", "{meta(2)},{meta(number)}"],
        &*FASTA,
        out,
    );
    let msg = "Column 'somecol' not found";
    fails(
        &[".", "-m", &meta, "--to-tsv", "{meta(somecol)}"],
        &*FASTA,
        msg,
    );
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
    with_tmpdir("st_meta_unordered_", |td| {
        let meta = td.file(".tsv", META_UNORDERED);
        let out = "2\n1\n10\n11\n";
        cmp(&[".", "-m", &meta, "--to-tsv", "{meta(2)}"], &*FASTA, out);

        let meta = td.file(".tsv", META_UNORDERED2);
        let out = "2\n1\n10\n11\n";
        cmp(&[".", "-m", &meta, "--to-tsv", "{meta(2)}"], &*FASTA, out);
    });
}

static META_MISSING: &str = "
seq3\t10
seq1\t2
seq2\t11";

#[test]
fn meta_missing() {
    with_tmpdir("st_meta_missing_", |td| {
        let meta = td.file(".tsv", META_MISSING);

        // opt_meta
        let out = &format!("2\n{NA}\n10\n11\n");
        cmp(
            &[".", "-m", &meta, "--to-tsv", "{opt_meta(2)}"],
            &*FASTA,
            out,
        );
        // has_meta
        let out = "true\nfalse\ntrue\ntrue\n";
        cmp(&[".", "-m", &meta, "--to-tsv", "{has_meta}"], &*FASTA, out);
        // meta() should fail
        let msg = "not found in metadata";
        fails(&[".", "-m", &meta, "--to-tsv", "{meta(2)}"], &*FASTA, msg);

        // undefined value
        use crate::helpers::NA;
        let meta = td.file(".tsv", &format!("id1\t{NA}\n"));
        cmp(
            &[".", "-m", &meta, "--to-csv", "id,opt_meta(2)"],
            ">id1\nSEQ\n",
            &format!("id1,{NA}\n"),
        );
        fails(
            &[".", "-m", &meta, "--to-csv", "id,meta(2)"],
            ">id1\nSEQ\n",
            &format!("field no. 2 in record 'id1' is '{NA}', which is reserved for missing values"),
        );
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
    let meta = tmp_file("st_meta_dup_", ".tsv", META_DUPLICATED);

    let out = "2\n1\n10\n11\n";
    cmp(
        &[".", "-m", &meta, "--to-tsv", "{meta(2)}", "--dup-ids"],
        &*FASTA,
        out,
    );
    let msg = "Found duplicate IDs in associated metadata (first: seq1)";
    cmp_stderr(
        &[".", "-m", &meta, "--to-tsv", "{meta(2)}"],
        &*FASTA,
        out,
        msg,
    );
}

#[test]
fn meta_multi_file() {
    with_tmpdir("st_meta_multi_", |td| {
        let m1 = td.file(".tsv", META_UNORDERED);
        let m2 = td.file(".tsv", META_HEADER);
        let m3 = td.file(".tsv", META_MISSING);

        let fields = "meta(2, 1),meta(2, 2),opt_meta(2, 3)";
        let out = &format!("2\t2\t2\n1\t1\t{NA}\n10\t10\t10\n11\t11\t11\n");
        cmp(
            &[".", "-m", &m1, "-m", &m2, "-m", &m3, "--to-tsv", fields],
            &*FASTA,
            out,
        );
        let fields = "meta(2, 1),meta(2, 2),meta(2, 3)";
        let msg = "not found in metadata";
        fails(
            &[".", "-m", &m1, "-m", &m2, "-m", &m3, "--to-tsv", fields],
            &*FASTA,
            msg,
        );
        // invalid index
        let msg = "Invalid metadata file no. requested: 0";
        fails(&[".", "-m", &m1, "--to-tsv", "has_meta(0)"], &*FASTA, msg);
        let msg = "Metadata file no. 2 was requested";
        fails(&[".", "-m", &m1, "--to-tsv", "meta(1, 2)"], &*FASTA, msg);
    });
}

/// Test missing and duplicated IDs with larger input
#[test]
fn meta_larger() {
    with_tmpdir("st_meta_large_", |td| {
        // For IDs 1-4, sequences and metadata are in same order -> 'in-sync' mode,
        // duplicates not detected.
        // 7 breaks the order -> switch to hash map index
        // From the remaining, 2 and 5 have no metadata entry in the HashMap
        //   (5 totally missing, 2 was read in 'sync mode');
        let ids = [1, 2, 2, 3, 4, 7, 2, 5, 6];
        let _meta = [(1, 1), (2, 2), (2, 0), (3, 3), (4, 4), (6, 6), (7, 7)];
        // expected output
        let out = [
            "1\t1",
            "2\t2",
            "2\t0",
            "3\t3",
            "4\t4",
            "7\t7",
            &format!("2\t{NA}"),
            &format!("5\t{NA}"),
            "6\t6",
        ]
        .join("\n")
            + "\n";
        // with --dup-ids: always the same value for 2
        let idx_out = [
            "1\t1",
            "2\t2",
            "2\t2",
            "3\t3",
            "4\t4",
            "7\t7",
            "2\t2",
            &format!("5\t{NA}"),
            "6\t6",
        ]
        .join("\n")
            + "\n";
        let fasta = ids.iter().map(|i| format!(">{i}\nSEQ\n")).join("");
        let meta = _meta.iter().map(|(i, m)| format!("{i}\t{m}\n")).join("");

        let fields = "id,opt_meta(2)";
        let dup_m = "Found duplicate IDs in associated metadata (first: 2)";
        let meta_f = td.file(".tsv", &meta);

        // 'hybrid' in-sync and hash map index parsing: duplication of no. 2 is not recognized
        cmp(&[".", "-m", &meta_f, "--to-tsv", fields], &fasta, &out);
        // using hash map index from start: duplicate entry is recognized
        cmp_stderr(
            &[".", "-m", &meta_f, "--to-tsv", fields, "--dup-ids"],
            &fasta,
            &idx_out,
            dup_m,
        );
        // 'meta' fails because of missing data
        let msg = "ID '2' not found in metadata";
        fails(&[".", "-m", &meta_f, "--to-tsv", "id,meta(2)"], &fasta, msg);
        // adding another sequence with ID=5 fails without --dup-ids
        // *note*: adding ID=7 would not fail, since the entry that breaks the
        // 'sync reading' is not checked for duplication.
        let fasta2 = format!("{fasta}>5\nSEQ\n");
        let dup_err = "Found duplicate sequence ID: '5'.";
        fails(&[".", "-m", &meta_f, "--to-tsv", fields], &fasta2, dup_err);
        let exp = format!("{idx_out}5\t{NA}\n");
        cmp(
            &[".", "-m", &meta_f, "--to-tsv", fields, "--dup-ids"],
            &fasta2,
            &exp,
        );

        // adding a duplicate metadata entry with ID=7 yields a message
        // even without --dup-ids
        let meta2 = format!("{meta}7\t700\n");
        let meta_f = td.file(".tsv", &meta2);
        // in this case, the duplicate ID = 7 is recognized even without --dup-ids
        // (previous entry with ID = 7 was read while in 'hash map mode');
        let dup_msg = "Found duplicate IDs in associated metadata (first: 7).";
        cmp_stderr(
            &[".", "-m", &meta_f, "--to-tsv", fields],
            &fasta,
            &out,
            dup_msg,
        );
    });
}

#[test]
#[cfg(feature = "gz")]
fn meta_compressed() {
    with_tmpdir("st_meta_compr_", |td| {
        let out = td.path("compr_meta.csv.gz");
        succeeds(&[".", "--outfields", "id,attr(p)", "-o", &out], &*FASTA);
        assert_eq!(&out.gz_content(), "seq1,2\nseq0,1\nseq3,10\nseq2,11\n");
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
