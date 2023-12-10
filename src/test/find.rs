use seq_io::fasta;

use super::*;

#[test]
fn exact_filter() {
    Tester::new()
        // filter
        .cmp(
            &["find", "-f", "GGCAGGCC"],
            *FASTA,
            &select_fasta(&[0, 1, 2]),
        )
        // exclude
        .cmp(&["find", "-e", "GGCAGGCC"], *FASTA, &select_fasta(&[3]));
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
        .cmp(&["find", "-if", "seq1"], *FASTA, &select_fasta(&[0]))
        .cmp(&["find", "--desc", "-f", "p="], *FASTA, &FASTA);
}

#[test]
fn regex() {
    Tester::new()
        .cmp(
            &["find", "--desc", "-rf", r"p=\d$"],
            *FASTA,
            &select_fasta(&[0, 1]),
        )
        .cmp(
            &["find", "-rf", "C[AT]GGCAGG"],
            *FASTA,
            &select_fasta(&[1, 2]),
        );
}

#[test]
fn multiline_seq() {
    Tester::new().cmp(&["find", "-f", "ATGC"], ">id\nAT\nGC\n", ">id\nATGC\n");
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
        assert_eq!(&s, ">seq1 m=\nSEQ1\n>seq3 m=\nSEQ3\n");
    })
}

#[test]
fn rng() {
    Tester::new()
        .cmp(
            &["find", "-f", "--rng", "..4", "TTGG"],
            *FASTA,
            &select_fasta(&[0]),
        )
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
        .cmp(&["find", "-r", "C[GC](A[AT])", "--to-tsv",
            "id,match,match(1),match(2),match(3),match(all),match_range(all),match_end(all),match_group(1),match_group(1,2)"], fasta,
            "seq\tCCAA\tCCAA\tCGAT\t\tCCAA,CGAT\t9-12,16-19\t12,19\tAA\tAT\n"
        )
        .cmp(&["find", "CAGG", "--to-csv",
            "id,match,match_start,match_end,match_range,match_neg_start,match_neg_end,match_drange,match_neg_drange,pattern_name,match_dist,match(all)"], fasta,
            "seq,CAGG,5,8,5-8,-21,-18,5..8,-21..-18,<pattern>,0,CAGG\n"
        );
}
//
// #[test]
// fn fuzzy() {
//     // compare seqtool output with equivalent code using rust-bio functions directly
//
//     let seq = "GCACCGTGGATGAGCGCCATAG";
//     let pattern = "ACC";
//     let fasta = format!(">seq\n{}\n", seq);
//     let vars = "match_range(all),match(all),match_dist(all)";
//
//     let t = Tester::new();
//
//     for max_dist in 0..2 {
//         // approximative matching
//         let mut ranges = vec![];
//         let mut matches = vec![];
//         let mut dists = vec![];
//         let m = fuzzy_find(pattern.as_bytes(), seq.as_bytes(), max_dist);
//         for (start, end, dist) in m {
//             ranges.push(format!("{}-{}", start + 1, end));
//             matches.push(seq[start..end].to_string());
//             dists.push(format!("{}", dist));
//         }
//
//         let d = format!("{}", max_dist);
//         let expected = format!(
//             "{}\t{}\t{}\n",
//             ranges.join(","),
//             matches.join(","),
//             dists.join(",")
//         );
//
//         t.cmp(
//                 &["find", "-g", "yes", "-d", &d, "--algo", "ukkonen", "--to-tsv", vars, pattern],
//                 &fasta, &expected
//             )
//             .cmp(
//                 &["find", "-g", "yes", "-d", &d, "--algo", "myers", "--to-tsv", vars, pattern],
//                 &fasta, &expected
//             );
//
//         // exact matches
//         if max_dist == 0 {
//             t.cmp(&["find", "--to-tsv", vars, pattern], &fasta, &expected)
//              .cmp(&["find", "-r", "--to-tsv", vars, pattern], &fasta, &expected);
//         }
//     }
// }
//
// // this code is equivalent to what seqtool should do
// // 1. find end positions up to edit distance of 'dist'
// // 2. NW alignment for finding the start position
// fn fuzzy_find(pattern: &[u8], text: &[u8], max_dist: usize) -> Vec<(usize, usize, usize)> {
//     use bio::pattern_matching::ukkonen;
//     use bio::alignment::pairwise;
//     use std::cmp::min;
//
//     // matcher
//     let mut u = ukkonen::Ukkonen::with_capacity(pattern.len(), ukkonen::unit_cost);
//     // aligner
//     let aln_score = |a, b| if a == b { 1 } else { -1 };
//     let mut s = pairwise::Scoring::new(-1, -1, &aln_score);
//     s.xclip_prefix = pairwise::MIN_SCORE;
//     s.xclip_suffix = pairwise::MIN_SCORE;
//     s.yclip_prefix = 0;
//     s.yclip_suffix = pairwise::MIN_SCORE;
//     let mut a = pairwise::Aligner::with_scoring(s);
//
//     let g =
//         // find end positions of hits
//         u.find_all_end(pattern, text, max_dist).map(|(end, dist)| {
//             // align subsequence with length of pattern + max. edit distance + 1
//             let end = end + 1;
//             let check_start = end - min(end, pattern.len() + dist as usize + 1);
//             let aln = a.custom(pattern, &text[check_start..end]);
//             (check_start + aln.ystart, end, dist)
//         })
//         // remove redundant hits per starting position
//         .group_by(|&(start, _, _)| start);
//
//     g.into_iter()
//         .map(|(_, mut it)| {
//             let mut out = None;
//             let mut best_dist = std::usize::MAX;
//             while let Some(m) = it.next() {
//                 if m.2 < best_dist {
//                     best_dist = m.2;
//                     out = Some(m.clone());
//                 }
//             }
//             out.unwrap()
//         })
//         .collect()
// }

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
            &["find", "--to-csv", vars, "--dist", "0", &subseq_indel],
            &fasta,
            ",\n",
        )
        .cmp(
            &["find", "--to-csv", vars, "--dist", "1", &subseq_indel],
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
            "seq,\n",
        )
        // fuzzy matching however will work
        .cmp(
            &[
                "find",
                "--to-csv",
                "id,match_range",
                "--dist",
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

        let vars = "match_range,match_range(1,1),match_range(1,2),match_range(1,3),match_dist,match_dist(1,1),match_dist(1,2),match_dist(1,3),pattern_name,pattern_name(1),pattern_name(2),pattern_name(3)";
        let out = "2-21,2-21,2-21,2-21,0,0,1,2,p0,p0,p2,p1\n";

        t.cmp(&["find", "--to-csv", vars, "-d2", "--algo", "myers", &patt_path], &fasta, out);
    });
}
