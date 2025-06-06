use super::*;

#[test]
fn filter() {
    let fa = ">id\nSEQ\n>id2 a=20\nSEQ\n>id3 a=\nSEQ";
    Tester::new()
        .cmp(
            &["filter", "seqlen > ungapped_seqlen && attr('p') >= 10"],
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
                "opt_attr('a') != undefined && opt_attr('a') >= 20",
                "--to-tsv",
                "id",
            ],
            fa,
            "id2\n",
        )
        .cmp(
            &["filter", "opt_attr('a') >= 20", "--to-tsv", "id"],
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
        let p = d.path().join("dropped.csv");
        let input = "@id1\nSEQ\n+\nJJJ\n@id2\nOTHER\n+\nJJJJJ\n";
        let cmd = &[
            "filter",
            "seq != 'SEQ'",
            "--fq",
            "--to-csv",
            "id,seq_num,seq",
            "--dropped",
            p.to_str().unwrap(),
        ];
        t.cmp(cmd, input, "id2,2,OTHER\n");
        #[cfg(any(feature = "all-commands", feature = "pass"))]
        t.cmp(
            &[".", "--fields", "id,desc,seq"],
            FileInput(cmd.last().unwrap()),
            "id1,1,SEQ\n",
        );
    })
}
