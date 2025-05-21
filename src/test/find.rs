use seq_io::fasta;

use crate::helpers::NA;

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
    let t = Tester::new();
    t.cmp(&["find", "-drf", r"p=\d$"], *FASTA, records!(0, 1))
        .cmp(&["find", "-rf", "C[AT]GGCAGG"], *FASTA, records!(1, 2))
        // case-sensitivity
        .cmp(&["find", "-rf", "C[aT]GGcAGG"], *FASTA, "")
        .cmp(&["find", "-crf", "C[aT]GGcAGG"], *FASTA, records!(1, 2))
        // UTF-8
        .cmp(&["find", "-rif", "^.$"], ">ä\nSEQ\n", ">ä\nSEQ\n");

    // groups
    let pat = r"(?:[a-z]+)(\d+?)\|(?<code>\w+?)\|";
    let fa = ">id123|abc|rest desc\nSEQ\n";
    let vars = "id,match,match_group(1),match_group(code)";
    let exp = "id123|abc|rest,id123|abc|,123,abc\n";
    t.cmp(&["find", "-ri", pat, "--to-csv", vars], fa, exp)
        .fails(
            &["find", "-ri", pat, "--to-csv", "match_group(b)"],
            fa,
            "Named regex group 'b' not present",
        )
        .fails(
            &["find", "-ri", pat, "--to-csv", "match_group(3)"],
            fa,
            "Regex group no. 3 not found",
        )
        .fails(
            &["find", "-i", pat, "--to-csv", "match_group(1)"],
            fa,
            "groups other than '0' (the whole hit) are not supported",
        );
    // multiple groups
    let fa = concat!(">s1\nSEQ\n", ">2\nSEQ\n", ">s\nSEQ\n");
    let pat = concat!(">a\n(?<s>.)(?<n>\\d+)\n", ">b\n^(?<n>\\d+)$\n");
    t.temp_file("patterns.fasta", Some(pat), |p, _| {
        let f = format!("file:{}", p);
        t.cmp(
            &[
                "find",
                "-ri",
                &f,
                "--to-csv",
                "id,pattern_name,match_group(1)",
            ],
            fa,
            &format!("s1,a,s\n2,b,2\ns,{na},{na}\n", na = NA),
        )
        .fails(
            &["find", "-ri", &f, "--to-csv", "id,match_group(s)"],
            fa,
            r"Named regex group 's' not present in pattern '^(?<n>\d+)$'",
        )
        .fails(
            &["find", "-ri", &f, "--to-csv", "id,match_group(n)"],
            fa,
            "Named group 'n' does not resolve to the same group number",
        );
    })
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
        &format!(">id pos={}\n\n", NA),
    );
}

#[test]
fn range() {
    let fa = ">id\nTAG\n";
    let v = "match_range";
    Tester::new()
        .cmp(&["find", "A", "--to-csv", v], fa, "2:2\n")
        .cmp(&["find", "A", "--rng", "2:2", "--to-csv", v], fa, "2:2\n")
        .cmp(
            &["find", "A", "--rng", ":1", "--to-csv", v],
            fa,
            &format!("{}\n", NA),
        );
}

#[test]
fn anchor() {
    let fa = ">id\nTATGCAGCA\n";
    let v = "match_range";
    Tester::new()
        .cmp(
            &["find", "TG", "--anchor-start", "1", "--to-csv", v],
            fa,
            &format!("{NA}\n"),
        )
        .cmp(
            &["find", "TG", "--anchor-start", "2", "--to-csv", v],
            fa,
            "3:4\n",
        )
        .cmp(
            &[
                "find",
                "TG",
                "--rng",
                "3:",
                "--anchor-start",
                "0",
                "--to-csv",
                v,
            ],
            fa,
            "3:4\n",
        )
        .cmp(
            &["find", "TG", "--anchor-end", "4", "--to-csv", v],
            fa,
            &format!("{NA}\n"),
        )
        .cmp(
            &["find", "TG", "--anchor-end", "5", "--to-csv", v],
            fa,
            "3:4\n",
        )
        .cmp(
            &[
                "find",
                "TG",
                "--rng",
                "1:5",
                "--anchor-end",
                "0",
                "--to-csv",
                v,
            ],
            fa,
            &format!("{NA}\n"),
        )
        .cmp(
            &[
                "find",
                "TG",
                "--rng",
                "1:5",
                "--anchor-end",
                "1",
                "--to-csv",
                v,
            ],
            fa,
            "3:4\n",
        );

    Tester::new()
        // TATGCAGCA
        //   TGCG
        .cmp(
            &[
                "find",
                "TGCG",
                "-D",
                "1",
                "--anchor-start",
                "2",
                "--to-csv",
                "match_range,aligned_pattern",
            ],
            fa,
            "3:6,TGCG\n",
        )
        .cmp(
            &[
                "find",
                "TGCG",
                "-D",
                "1",
                "--anchor-end",
                "2",
                "--to-csv",
                "match_range",
            ],
            fa,
            &format!("{NA}\n"),
        )
        // TATGCAGCA
        //   TGC-G
        .cmp(
            &[
                "find",
                "TGCG",
                "-D",
                "1",
                "--hit-scoring",
                "2,-1,-1",
                "--anchor-start",
                "2",
                "--to-csv",
                "match_range,aligned_pattern",
            ],
            fa,
            "3:7,TGC-G\n",
        )
        .cmp(
            &[
                "find",
                "TGCG",
                "-D",
                "1",
                "--hit-scoring",
                "2,-1,-1",
                "--anchor-end",
                "2",
                "--to-csv",
                "match_range,aligned_pattern",
            ],
            fa,
            "3:7,TGC-G\n",
        );
}

#[test]
fn drop_file() {
    let t = Tester::new();
    let input = ">seq1\nSEQ1\n>seq2\nSEQ2\n>seq3\nSEQ3\n";
    t.temp_dir("find_drop", |d| {
        // FASTA
        let out_fa = ">seq2 m=4:4\nSEQ2\n";
        let dropped_fa = format!(">seq1 m={na}\nSEQ1\n>seq3 m={na}\nSEQ3\n", na = NA);
        let p = d.path().join("dropped.fa");
        let cmd = &mut [
            "find",
            "-f",
            "2",
            "-a",
            "m={match_range}",
            "--dropped",
            p.to_str().unwrap(),
        ];
        t.cmp(cmd, input, out_fa);
        t.cmp(&["."], FileInput(cmd.last().unwrap()), &dropped_fa);
        let p = d.path().join("dropped.fasta.gz");
        *cmd.last_mut().unwrap() = p.to_str().unwrap();
        t.cmp(cmd, input, out_fa);
        t.cmp(
            &[".", "--fmt", "fasta.gz"],
            FileInput(cmd.last().unwrap()),
            &dropped_fa,
        );

        // TSV
        #[cfg(feature = "gz")]
        {
            let dropped_tsv = format!("seq1\t{na}\tSEQ1\nseq3\t{na}\tSEQ3\n", na = NA);
            let p = d.path().join("dropped.tsv.gz");
            let cmd = &mut [
                "find",
                "-f",
                "2",
                "-a",
                "m={match_range}",
                "--outfields",
                "id,match_range,seq",
                "--dropped",
                p.to_str().unwrap(),
            ];
            t.cmp(cmd, input, out_fa);
            t.cmp(
                &[".", "--fields", "id,desc,seq"],
                FileInput(cmd.last().unwrap()),
                &dropped_tsv,
            );
        }
    })
}

#[test]
fn rng() {
    Tester::new()
        .cmp(&["find", "-f", "--rng", ":4", "TTGG"], *FASTA, records!(0))
        .cmp(&["find", "-f", "--rng", ":3", "TTGG"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", "2:5", "TTGG"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", "2:4", "TGGC"], *FASTA, "")
        .cmp(&["find", "-f", "--rng", "-5:", "GATCA"], *FASTA, &FASTA)
        .cmp(&["find", "-f", "--rng", "16:-7", "CGAT"], *FASTA, &FASTA);
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
            &format!(
                "seq,C[GC](A[AT]),CCAA,CCAA,CCAA,CGAT,{},CCAA,CGAT,9:12,16:19,12,19,AA,AT\n",
                NA
            ),
        )
        .cmp(
            &[
                "find",
                "CAGG",
                "--to-csv",
                "id,pattern,match,aligned_match,\
                match_start,match_end,match_range,match_neg_start,match_neg_end,\
                pattern_name,match_diffs,match(all)",
            ],
            fasta,
            "seq,CAGG,CAGG,CAGG,5,8,5:8,-21,-18,<pattern>,0,CAGG\n",
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
    // There are two possible alignments with the same edit distance:
    //
    // (1) alignment with lowest possible end coordinate:
    //
    // AACG--CCACT  [diffs = 2]
    //  |||xx|
    //  ACGTGC
    //
    // (2) optimal alignment if gaps are penalized
    //
    // AACGCACT  [diffs = 2]
    //  |||\\|
    //  ACGTGC
    //
    // with gap penalty of < -1, the second alignment will be chosen,
    // with penalty of -1 it will be the first one
    let v = "match,aligned_match,aligned_pattern,match_range,match_len,match_diffs";

    Tester::new()
        // don't penalize gaps more than substitutions
        // -> alignment (1) is chosen since it has the lowest end coordinate (= 2 insertions in text)
        .cmp(
            &[
                "find",
                "-D2",
                "--hit-scoring",
                "1,-1,-1",
                "--to-csv",
                v,
                pattern,
            ],
            fa,
            "ACGC,ACG--C,ACGTGC,2:5,4,2\n",
        )
        // penalize gaps (gap penalty of -2 is the default)
        // -> ungapped alignment is chosen
        .cmp(
            &["find", "-D2", "--to-csv", v, pattern],
            fa,
            "ACGCAC,ACGCAC,ACGTGC,2:7,6,2\n",
        )
        // no alignment, only coordinates and edit distance
        // (internally this switches to another `Hit` implementation, see src/cmd/find/matcher/approx.rs)
        .cmp(
            &[
                "find",
                "-D2",
                "--to-csv",
                "match_range,match_diffs",
                pattern,
            ],
            fa,
            "2:7,2\n",
        )
        // edit distance only
        // (this is the fastest implementation)
        .cmp(
            &["find", "-D2", "--to-csv", "match_diffs", pattern],
            fa,
            "2\n",
        )
        // report *all* hits with edit distance <= 2
        // (again switches to another implementation)
        .cmp(
            &[
                "find",
                "-D2",
                "--hit-scoring",
                "1,-1,-1",
                "--to-tsv",
                "match_range(all),aligned_match(all)",
                pattern,
            ],
            fa,
            "2:5,15:18\tACG--C,ACG--C\n",
        )
        // ungapped alignment
        .cmp(
            &[
                "find",
                "-D2",
                "--to-tsv",
                "match_range(all),aligned_match(all)",
                pattern,
            ],
            fa,
            "2:7,15:20\tACGCAC,ACGCAC\n",
        )
        // no alignment, only coordinates / edit distance
        .cmp(
            &[
                "find",
                "-D2",
                "--to-tsv",
                "match_range(all),match_diffs(all)",
                pattern,
            ],
            fa,
            "2:7,15:20\t2,2\n",
        );
}

#[test]
fn ambig() {
    let seq = "AACACACTGTGGAGTTTTCAT";
    //              R        N
    let subseq = "ACRCTGTGGAGNTTTC";
    let subseq_indel = "ACRCTG-GGAGNTTTC".replace('-', "");
    let vars = "match_range,match";
    let expected = "4:19,ACACTGTGGAGTTTTC\n";
    let fasta = format!(">seq\n{}\n", seq);

    Tester::new()
        .cmp(&["find", "--to-csv", vars, subseq], &fasta, expected)
        .cmp(&["find", "--to-csv", vars, subseq], &fasta, expected)
        .cmp(
            &["find", "--to-csv", vars, "-D", "0", &subseq_indel],
            &fasta,
            &format!("{na},{na}\n", na = NA),
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
            "seq,2:16\n",
        )
        .cmp(
            &["find", "--to-csv", "id,match_range", &seq_orig_[1..]],
            &*format!(">seq\n{}\n", seq_ambig),
            &format!("seq,{}\n", NA),
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
            "seq,2:16\n",
        );
}

#[test]
fn case_insensitive() {
    let fasta = ">id\nAACaCacTGTGGAGTTTTCAT\n";

    Tester::new()
        .cmp(
            &["find", "--to-csv", "match_range", "CaCacT"],
            fasta,
            "3:8\n",
        )
        .cmp(
            &["find", "--to-csv", "match_range", "CACACT"],
            fasta,
            &format!("{NA}\n"),
        )
        .cmp(
            &["find", "-c", "--to-csv", "match_range,match", "CACACt"],
            fasta,
            "3:8,CaCacT\n",
        )
        .cmp(
            &["find", "-c", "--to-csv", "match_range,match", "cAcact"],
            fasta,
            "3:8,CaCacT\n",
        )
        .cmp(
            &["find", "--to-csv", "match_range,match", "CrCacY"],
            fasta,
            "3:8,CaCacT\n",
        )
        .cmp(
            &[
                "find",
                "--no-ambig",
                "--to-csv",
                "match_range,match",
                "CrCacY",
            ],
            fasta,
            &format!("{NA},{NA}\n"),
        )
        .cmp(
            &["find", "--to-csv", "match_range,match", "cRcAYT"],
            fasta,
            &format!("{NA},{NA}\n"),
        )
        .cmp(
            &["find", "-c", "--to-csv", "match_range,match", "cRcAYT"],
            fasta,
            "3:8,CaCacT\n",
        )
        .cmp(
            &[
                "find",
                "-c",
                "-D",
                "1",
                "--to-csv",
                "match_range,match_ins",
                "acrCTGGGagnttTC",
            ],
            ">id\nACRCTGTGGAGNTTTC\n",
            "1:16,1\n",
        )
        .cmp(
            &[
                "find",
                "-c",
                "--to-csv",
                "match_range",
                "--seqtype",
                "other",
                "AbCdEfGhIjKlMnOpQrStUvWxYz",
            ],
            ">id\naBcDeFgHiJkLmNoPqRsTuVwXyZ\n",
            "1:26\n",
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

        let vars =
            "match_range,match_range(1,1),match_range(1,2),match_range(1,3),match_range(1,1,'-'),\
                match_diffs,match_diffs(1,1),match_diffs(1,2),match_diffs(1,3),\
                pattern_name,pattern_name(1),pattern_name(2),pattern_name(3)";
        let out = "2:21,2:21,2:21,2:21,2-21,0,0,1,2,p0,p0,p2,p1\n";

        t.cmp(&["find", "--to-csv", vars, "-D2", &patt_path], &fasta, out);
    });
}
