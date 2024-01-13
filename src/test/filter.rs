use super::*;

#[test]
fn filter() {
    let fa = ">id\nSEQ\n>id2 a=20\nSEQ\n>id3 a=\nSEQ";
    Tester::new()
        .cmp(
            &["filter", "seqlen > ungapped_seqlen && attr(p) >= 10"],
            *FASTA,
            &SEQS[2..].concat(),
        )
        .cmp(&["filter", "id == 'seq0'"], *FASTA, SEQS[1])
        .cmp(&["filter", "id == undefined"], *FASTA, "")
        // note: comparison with undefined in Javascript returns false, thus only sequences
        // with defined attributes are kept
        .cmp(
            &[
                "filter",
                "opt_attr(a) != undefined && opt_attr(a) >= 20",
                "--to-tsv",
                "id",
            ],
            fa,
            "id2\n",
        )
        .cmp(
            &["filter", "opt_attr(a) >= 20", "--to-tsv", "id"],
            fa,
            "id2\n",
        )
        // Javascript Regex:
        // currently /regex/ syntax with strings matching any variable/function
        // cannot be handled
        // .cmp(
        //     &["filter", r"(/id\d+/).test(id)", "--to-tsv", "id"],
        //     fa,
        //     "id2\nid3\n",
        // )
        .cmp(
            &[
                "filter",
                r"(new RegExp('id\\d+')).test(id)",
                "--to-tsv",
                "id",
            ],
            fa,
            "id2\nid3\n",
        );
}

#[test]
fn drop_file() {
    let t = Tester::new();
    t.temp_dir("find_drop", |d| {
        let out = d.path().join("dropped.fa");
        let out_path = out.to_str().expect("invalid path");

        let fa = ">id1\nSEQ\n>id2\nOTHER";
        t.cmp(
            &[
                "filter",
                "seq != 'SEQ'",
                "-a",
                "i={num}",
                "--dropped",
                out_path,
            ],
            fa,
            ">id2 i=2\nOTHER\n",
        );

        let mut f = File::open(out_path).expect("File not there");
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();

        assert_eq!(&s, ">id1 i=1\nSEQ\n");
    })
}
