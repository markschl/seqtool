use seq_io::fasta;

use super::*;

#[test]
fn exact_filter() {
    Tester::new()
        // filter
        .cmp(&["find", "-f", "GGCAGGCC"], *FASTA, records!(0, 1, 2))
        // exclude
        .cmp(&["find", "-e", "GGCAGGCC"], *FASTA, records!(3))
        // nothing: should fail
        .fails(&["find", "GGCAGGCC"], *FASTA, "Find command does nothing");
}

#[test]
fn replace() {
    let fasta = ">seq_123 desc\nATGC\n";
    Tester::new()
        .cmp(
            &["find", "GC", "--rep", "??"],
            fasta,
            ">seq_123 desc\nAT??\n",
        )
        .cmp(
            &[
                "find",
                "-ir",
                r"\w+_(\d+)",
                "--rep",
                "new_name_{match_group(1)}",
            ],
            fasta,
            ">new_name_123 desc\nATGC\n",
        )
        .cmp(
            &["find", "--desc", "desc", "--rep", "????"],
            fasta,
            ">seq_123 ????\nATGC\n",
        );
}

#[test]
fn id_desc() {
    Tester::new()
        .cmp(&["find", "-if", "seq1"], *FASTA, records!(0))
        .cmp(&["find", "--desc", "-f", "p="], *FASTA, &FASTA);
}

#[test]
fn regex() {
    Tester::new()
        .cmp(&["find", "-drf", r"p=\d$"], *FASTA, records!(0, 1))
        .cmp(&["find", "-rf", "C[AT]GGCAGG"], *FASTA, records!(1, 2))
        // UTF-8
        .cmp(&["find", "-rif", "^.$"], ">ä\nSEQ\n", ">ä\nSEQ\n");
}

#[test]
fn multiline_seq() {
    Tester::new().cmp(&["find", "-f", "ATGC"], ">id\nAT\nGC\n", ">id\nATGC\n");
}

#[test]
fn missing() {
    Tester::new().cmp(
        &["find", "ATGC", "-a", "pos={match_start}"],
        ">id\n\n",
        ">id pos=N/A\n\n",
    );
}

#[test]
fn range() {
    let fa = ">id\nTAG\n";
    let v = "match_range";
    Tester::new()
        .cmp(&["find", "A", "--to-csv", v], fa, "2-2\n")
        .cmp(&["find", "A", "--rng", "2..2", "--to-csv", v], fa, "2-2\n")
        .cmp(&["find", "A", "--rng", "..1", "--to-csv", v], fa, "N/A\n")
        .cmp(
            &["find", "G", "--max-shift-start", "2", "--to-csv", v],
            fa,
            "3-3\n",
        )
        .cmp(
            &[
                "find",
                "G",
                "--rng",
                "2..",
                "--max-shift-start",
                "1",
                "--to-csv",
                v,
            ],
            fa,
            "3-3\n",
        )
        .cmp(
            &["find", "G", "--max-shift-start", "1", "--to-csv", v],
            fa,
            "N/A\n",
        );
}

#[test]
fn drop_file() {
    let t = Tester::new();
    let fa = ">seq1\nSEQ1\n>seq2\nSEQ2\n>seq3\nSEQ3\n";
    t.temp_dir("find_drop", |d| {
        let out = d.path().join("dropped.fa");
        let out_path = out.to_str().expect("invalid path");

        t.cmp(
            &[
                "find",
                "-f",
                "2",
                "-a",
                "m={match_range}",
                "--dropped",
                out_path,
            ],
            fa,
            ">seq2 m=4-4\nSEQ2\n",
        );

        let mut f = File::open(out_path).expect("File not there");
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(&s, ">seq1 m=N/A\nSEQ1\n>seq3 m=N/A\nSEQ3\n");
    })
}

#[test]
fn rng() {
    Tester::new()
        .cmp(&["find", "-f", "--rng", "..4", "TTGG"], *FASTA, records!(0))
        .cmp(&["find", "-f", "--rng", "..3", "TTGG"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", "2..5", "TTGG"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", "2..4", "TGGC"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", " -5..", "GATCA"], *FASTA, &FASTA)
        .cmp(&["find", "-f", "--rng", "16..-7", "CGAT"], *FASTA, &FASTA);
}

#[test]
fn vars() {
    let fasta = ">seq\nTTGGCAGGCCAAGGCCGATGGATCA\n";
    Tester::new()
        .cmp(
            &[
                "find",
                "-r",
                "C[GC](A[AT])",
                "--to-csv",
                "id,pattern,match,aligned_match,match(1),match(2),match(3),match(all),\
                match_range(all),match_end(all),\
                match_group(1),match_group(1,2)",
            ],
            fasta,
            "seq,C[GC](A[AT]),CCAA,CCAA,CCAA,CGAT,N/A,CCAA,CGAT,9-12,16-19,12,19,AA,AT\n",
        )
        .cmp(
            &[
                "find",
                "CAGG",
                "--to-csv",
                "id,pattern,match,aligned_match,\
                match_start,match_end,match_range,match_neg_start,match_neg_end,\
                match_drange,match_neg_drange,\
                pattern_name,match_diffs,match(all)",
            ],
            fasta,
            "seq,CAGG,CAGG,CAGG,5,8,5-8,-21,-18,5..8,-21..-18,<pattern>,0,CAGG\n",
        );
}

// TODO: to be expanded
#[test]
fn fuzzy() {
    // sequence length = 20, pattern length = 5
    let fa = ">i\nAACACACTGTGGAGTTTTCA\n";
    //                 x  mismatch
    let pattern = "ACACC";
    let v = "match,aligned_match,match_diffs";

    let t = Tester::new();
    t.cmp(&["find", "-f", "--to-csv", v, pattern], fa, "")
        .cmp(
            &["find", "-f", "-D1", "--to-csv", v, pattern],
            fa,
            "ACACA,ACACA,1\n",
        )
        // rate (relative to pattern length)
        .cmp(
            &["find", "-f", "-R", "0.1999999", "--to-csv", v, pattern],
            fa,
            "",
        )
        .cmp(
            &["find", "-f", "-R", "0.2", "--to-csv", v, pattern],
            fa,
            "ACACA,ACACA,1\n",
        );
    // match/pattern alignment
    let fa = concat!(">s1\nACAATGG\n", ">s2\nACG\n", ">s3\nAAGGTA\n");
    let pat = concat!(">a\nCATG\n", ">b\nACGT\n");
    t.temp_file("patterns.fasta", Some(pat), |p, _| {
        let v = "id,pattern_name,pattern,aligned_pattern,pattern_len,\
                 match,aligned_match,match_len,\
                 match_diffs,match_ins,match_del,match_subst";
        let exp = concat!(
            "s1,a,CATG,CA-TG,4,CAATG,CAATG,5,1,1,0,0\n",
            "s2,b,ACGT,ACGT,4,ACG,ACG-,3,1,0,1,0\n",
            "s3,b,ACGT,ACGT,4,AGGT,AGGT,4,1,0,0,1\n",
        );
        t.cmp(
            &["find", "-D2", &format!("file:{}", p), "--to-csv", v],
            fa,
            exp,
        );
    })
}

#[test]
fn fuzzy_gaps() {
    // same sequence repeated twice (with a TTTTT spacer) to test multi-hit reporting
    let fa = ">i\nAACGCACTTTTTTAACGCACT\n";
    let pattern = "ACGTGC";
    // alignment is:
    //
    // AACG--CCACT  [diffs = 2]
    //  |||xx|
    //  ACGTGC
    //
    // or
    //
    // AACGCACT  [diffs = 2]
    //  |||\\|
    //  ACGTGC
    //
    // with gap penalty of > 0, the second alignment will be chosen,
    // with penalty of 0 it will be the first one since the end position
    let v = "match,aligned_match,aligned_pattern,match_range,match_len,match_diffs";

    Tester::new()
        .cmp(
            &[
                "find",
                "-f",
                "-D2",
                "--gap-penalty",
                "0",
                "--to-csv",
                v,
                pattern,
            ],
            fa,
            "ACGC,ACG--C,ACGTGC,2-5,4,2\n",
        )
        .cmp(
            &[
                "find",
                "-f",
                "-D2",
                "--gap-penalty",
                "2",
                "--to-csv",
                v,
                pattern,
            ],
            fa,
            "ACGCAC,ACGCAC,ACGTGC,2-7,6,2\n",
        )
        .cmp(
            &[
                "find",
                "-f",
                "-D2",
                "--gap-penalty",
                "0",
                "--to-tsv",
                "match_range(all),aligned_match(all)",
                pattern,
            ],
            fa,
            "2-5,15-18\tACG--C,ACG--C\n",
        )
        .cmp(
            &[
                "find",
                "-f",
                "-D2",
                "--gap-penalty",
                "1000",
                "--to-tsv",
                "match_range(all),aligned_match(all)",
                pattern,
            ],
            fa,
            "2-7,15-20\tACGCAC,ACGCAC\n",
        );
}

#[test]
fn ambig() {
    let seq = "AACACACTGTGGAGTTTTCAT";
    //              R        N
    let subseq = "ACRCTGTGGAGNTTTC";
    let subseq_indel = "ACRCTG-GGAGNTTTC".replace('-', "");
    let vars = "match_range,match";
    let expected = "4-19,ACACTGTGGAGTTTTC\n";
    let fasta = format!(">seq\n{}\n", seq);

    Tester::new()
        .cmp(&["find", "--to-csv", vars, subseq], &fasta, expected)
        .cmp(&["find", "--to-csv", vars, subseq], &fasta, expected)
        .cmp(
            &["find", "--to-csv", vars, "-D", "0", &subseq_indel],
            &fasta,
            "N/A,N/A\n",
        )
        .cmp(
            &["find", "--to-csv", vars, "-D", "1", &subseq_indel],
            &fasta,
            expected,
        );

    // matching is asymmetric
    let seq_orig_ = "ACACTGTGGAGTTTTC";
    //                 R        N
    let seq_ambig = "ACRCTGTGGAGNTTTC";
    // TODO: working around Ukkonen bug in rust-bio
    Tester::new()
        .cmp(
            &["find", "--to-csv", "id,match_range", &seq_ambig[1..]],
            &*format!(">seq\n{}\n", seq_orig_),
            "seq,2-16\n",
        )
        .cmp(
            &["find", "--to-csv", "id,match_range", &seq_orig_[1..]],
            &*format!(">seq\n{}\n", seq_ambig),
            "seq,N/A\n",
        )
        // fuzzy matching however will work
        .cmp(
            &[
                "find",
                "--to-csv",
                "id,match_range",
                "-D",
                "2",
                &seq_orig_[1..],
            ],
            &*format!(">seq\n{}\n", seq_ambig),
            "seq,2-16\n",
        );
}

#[test]
fn threaded() {
    for t in 1..4 {
        let mut cap = 3;
        while cap < t * FASTA.len() {
            Tester::new()
                .cmd(
                    &[
                        "find",
                        "-f",
                        "--id",
                        "--to-tsv",
                        "id",
                        "-t",
                        &format!("{}", t),
                        "--buf-cap",
                        &format!("{}", cap),
                        "seq",
                    ],
                    *FASTA,
                )
                .stdout(contains("seq0").from_utf8())
                .stdout(contains("seq1").from_utf8())
                .stdout(contains("seq2").from_utf8())
                .stdout(contains("seq3").from_utf8())
                .success();
            cap += 10;
        }
    }
}

#[test]
fn multiple() {
    let seq = "AACACACTGTGGAGTTTTCGA";
    let patterns = &[
        b"ACACACTGTGGAGTTTTCGA", // p0: 0 mismatches at end
        b"ACACACTGAGGAGTCTTCGA", // p1: 2 mismatches at end
        //            A     C
        b"ACACACTGAGGAGTTTTCGA", // p2: 1 mismatches at end
                                 //            A
    ];
    let fasta = format!(">seq\n{}\n", seq);

    let t = Tester::new();

    t.temp_file("patterns.fa", None, |p, mut f| {
        let patt_path = format!("file:{}", p);

        for (i, p) in patterns.iter().enumerate() {
            fasta::write_parts(&mut f, format!("p{}", i).as_bytes(), None, *p as &[u8]).unwrap();
        }

        let vars = "match_range,match_range(1,1),match_range(1,2),match_range(1,3),\
                match_diffs,match_diffs(1,1),match_diffs(1,2),match_diffs(1,3),\
                pattern_name,pattern_name(1),pattern_name(2),pattern_name(3)";
        let out = "2-21,2-21,2-21,2-21,0,0,1,2,p0,p0,p2,p1\n";

        t.cmp(&["find", "--to-csv", vars, "-D2", &patt_path], &fasta, out);
    });
}
