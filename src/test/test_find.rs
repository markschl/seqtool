use std::fs::File;
use itertools::Itertools;
use seq_io::fasta;
use assert_cli::Assert;

use super::*;

#[test]
fn find_exact_filter() {
    // filter
    cmp_stdout!(&["find", "-f", "GGCAGGCC"], FASTA, select(&[0, 1, 2]));
    // exclude
    cmp_stdout!(&["find", "-e", "GGCAGGCC"], FASTA, select(&[3]));
}

#[test]
fn find_replace() {
    let fasta = ">seq_123 desc\nATGC\n";
    cmp_stdout!(
        &["find", "GC", "--rep", "??"],
        fasta,
        ">seq_123 desc\nAT??\n"
    );
    cmp_stdout!(
        &[
            "find",
            "-ir",
            r"\w+_(\d+)",
            "--rep",
            "new_name_{f:match::1}"
        ],
        fasta,
        ">new_name_123 desc\nATGC\n"
    );
    cmp_stdout!(
        &["find", "--desc", "desc", "--rep", "????"],
        fasta,
        ">seq_123 ????\nATGC\n"
    );
}

#[test]
fn find_id_desc() {
    cmp_stdout!(&["find", "-if", "seq1"], FASTA, select(&[0]));
    cmp_stdout!(&["find", "--desc", "-f", "p="], FASTA, FASTA);
}

#[test]
fn find_regex() {
    cmp_stdout!(&["find", "--desc", "-rf", r"p=\d$"], FASTA, select(&[0, 1]));
    cmp_stdout!(&["find", "-rf", "C[AT]GGCAGG"], FASTA, select(&[1, 2]));
}

#[test]
fn find_rng() {
    cmp_stdout!(&["find", "-f", "--rng", "..4", "TTGG"], FASTA, select(&[0]));
    cmp_stdout!(&["find", "-f", "--rng", "..3", "TTGG"], FASTA, "");
    cmp_stdout!(&["find", "-f", "--rng", "2..5", "TTGG"], FASTA, "");
    cmp_stdout!(&["find", "-f", "--rng", "2..4", "TGGC"], FASTA, "");
    //cmp_stdout!(&["find", "-f", "--rng", "\" -4..\"", "GATCA"], FASTA, FASTA);
    cmp_stdout!(&["find", "-f", "--rng", "16..-7", "CGAT"], FASTA, FASTA);
}

#[test]
fn find_vars() {
    let fasta = ">seq\nTTGGCAGGCCAAGGCCGATGGATCA\n";
    cmp_stdout!(&["find", "-r", "C[GC](A[AT])", "--to-txt",
        "id,f:match,f:match:1,f:match:2,f:match:3,f:match:all,f:range:all,f:match::1,f:match:2:1"], fasta,
        "seq\tCCAA\tCCAA\tCGAT\t\tCCAA,CGAT\t9-12,16-19\tAA\tAT\n"
    );
    cmp_stdout!(&["find", "CAGG", "--to-csv",
        "id,f:match,f:start,f:end,f:range,f:neg_start,f:neg_end,f:drange,f:neg_drange,f:name,f:dist,f:match:all"], fasta,
        "seq,CAGG,5,8,5-8,-21,-18,5..8,-21..-18,,0,CAGG\n"
    );
}

#[test]
fn find_fuzzy() {
    // compare seqtool output with equivalent code using rust-bio functions directly

    let seq = "GCACCGTGGATGAGCGCCATAG";
    let pattern = "ACC";
    let fasta = format!(">seq\n{}\n", seq);
    let vars = "f:range:all,f:match:all,f:dist:all";

    for max_dist in 0..2 {
        // approximative matching
        let mut ranges = vec![];
        let mut matches = vec![];
        let mut dists = vec![];
        let m = fuzzy_find(pattern.as_bytes(), seq.as_bytes(), max_dist);
        for (start, end, dist) in m {
            ranges.push(format!("{}-{}", start + 1, end));
            matches.push(seq[start..end].to_string());
            dists.push(format!("{}", dist));
        }

        let d = format!("{}", max_dist);
        let expected = format!(
            "{}\t{}\t{}\n",
            ranges.join(","),
            matches.join(","),
            dists.join(",")
        );

        cmp_stdout!(
            &["find", "-g", "yes", "-d", &d, "--algo", "ukkonen", "--to-txt", vars, pattern],
            fasta, &expected
        );
        cmp_stdout!(
            &["find", "-g", "yes", "-d", &d, "--algo", "myers", "--to-txt", vars, pattern],
            fasta, &expected
        );

        // exact matches
        if max_dist == 0 {
            cmp_stdout!(&["find", "--to-txt", vars, pattern], fasta, &expected);
            cmp_stdout!(&["find", "-r", "--to-txt", vars, pattern], fasta, &expected);
        }
    }
}

// this code is equivalent to what seqtool should do
// 1. find end positions up to edit distance of 'dist'
// 2. SW alignment for finding the start position
fn fuzzy_find(pattern: &[u8], text: &[u8], max_dist: usize) -> Vec<(usize, usize, usize)> {
    use bio::pattern_matching::ukkonen;
    use bio::alignment::pairwise;
    use std::cmp::min;

    // matcher
    let mut u = ukkonen::Ukkonen::with_capacity(pattern.len(), ukkonen::unit_cost);
    // aligner
    let aln_score = |a, b| if a == b { 1 } else { -1 };
    let mut s = pairwise::Scoring::new(-1, -1, &aln_score);
    s.xclip_prefix = pairwise::MIN_SCORE;
    s.xclip_suffix = pairwise::MIN_SCORE;
    s.yclip_prefix = 0;
    s.yclip_suffix = pairwise::MIN_SCORE;
    let mut a = pairwise::Aligner::with_scoring(s);

    let g =
        // find end positions of hits
        u.find_all_end(pattern, text, max_dist).map(|(end, dist)| {
            // align subsequence with length of pattern + max. edit distance + 1
            let end = end + 1;
            let check_start = end - min(end, pattern.len() + dist as usize + 1);
            let aln = a.custom(pattern, &text[check_start..end]);
            (check_start + aln.ystart, end, dist)
        })
        // remove redundant hits per starting position
        .group_by(|&(start, _, _)| start);

    g.into_iter()
        .map(|(_, mut it)| {
            let mut out = None;
            let mut best_dist = ::std::usize::MAX;
            while let Some(m) = it.next() {
                if m.2 < best_dist {
                    best_dist = m.2;
                    out = Some(m.clone());
                }
            }
            out.unwrap()
        })
        .collect()
}

#[test]
fn find_ambig() {
    let seq = "AACACACTGTGGAGTTTTCAT";
    //                    R        N
    let subseq = "ACRCTGTGGAGNTTTC";
    let subseq_indel = "ACRCTG-GGAGNTTTC".replace("-", "");
    let vars = "f:range,f:match";
    let expected = "4-19,ACACTGTGGAGTTTTC\n";
    let fasta = format!(">seq\n{}\n", seq);

    cmp_stdout!(&["find", "--to-csv", vars, subseq], fasta, expected);
    cmp_stdout!(
        &["find", "--to-csv", vars, "--ambig", "yes", subseq],
        fasta,
        expected
    );
    cmp_stdout!(
        &["find", "--to-csv", vars, "--dist", "0", &subseq_indel],
        fasta,
        ",\n"
    );
    cmp_stdout!(
        &["find", "--to-csv", vars, "--dist", "1", &subseq_indel],
        fasta,
        expected
    );
    // gap open penalty doubled -> distance higher
    //cmp_stdout!(&["find", "--ambig", "--to-csv", vars, "--dist", "1", "--gap-penalties", "-2,-1", &subseq_indel], fasta, ",");

    // matching is asymmetric
    let seq_orig = "ACACTGTGGAGTTTTC";
    //                 R        N
    let seq_ambig = "ACRCTGTGGAGNTTTC";
    // TODO: working around Ukkonen bug in rust-bio
    cmp_stdout!(
        &[
            "find",
            "--to-csv",
            "id,f:range",
            "--ambig",
            "yes",
            &seq_ambig[1..]
        ],
        format!(">seq\n{}\n", seq_orig),
        "seq,2-16\n"
    );
    cmp_stdout!(
        &[
            "find",
            "--to-csv",
            "id,f:range",
            "--ambig",
            "yes",
            &seq_orig[1..]
        ],
        format!(">seq\n{}\n", seq_ambig),
        "seq,"
    );
    // fuzzy matching however will work
    cmp_stdout!(
        &[
            "find",
            "--to-csv",
            "id,f:range",
            "--ambig",
            "yes",
            "--dist",
            "2",
            &seq_orig[1..]
        ],
        format!(">seq\n{}\n", seq_ambig),
        "seq,2-16\n"
    );
}

#[test]
fn find_threaded() {
    for cap in 3..FASTA.len() * 4 {
        let cap = &format!("{}", cap);
        run!(&["find", "-f", "--id", "--to-txt", "id", "-t4", "--buf-cap", cap, "seq"], FASTA)
            .stdout()
            .contains("seq0")
            .stdout()
            .contains("seq1")
            .stdout()
            .contains("seq2")
            .stdout()
            .contains("seq3");
    }
}

#[test]
fn find_multiple() {
    let seq = "AACACACTGTGGAGTTTTCGA";
    let patterns = &[
        b"ACACACTGTGGAGTTTTCGA", // p0: 0 mismatches at end
        b"ACACACTGAGGAGTCTTCGA", // p1: 2 mismatches at end
        //            A     C
        b"ACACACTGAGGAGTTTTCGA", // p2: 1 mismatches at end
                                 //            A
    ];
    let fasta = format!(">seq\n{}\n", seq);

    let tmpdir = tempdir::TempDir::new("find_test").expect("Could not create temporary directory");
    let patt_file = tmpdir.path().join("patterns.fa");
    let patt_path = format!("file:{}", patt_file.to_str().unwrap());
    let mut f = File::create(patt_file).unwrap();
    for (i, p) in patterns.into_iter().enumerate() {
        fasta::write_parts(&mut f, format!("p{}", i).as_bytes(), None, *p as &[u8]).unwrap();
    }
    let vars = "f:range,f:range.1,f:range.2,f:range.3,f:dist,f:dist.1,f:dist.2,f:dist.3,f:name,f:name.1,f:name.2,f:name.3";
    let out = "2-21,2-21,2-21,2-21,0,0,1,2,p0,p0,p2,p1\n";
    cmp_stdout!(
        &["find", "--to-csv", vars, "-d2", "--algo", "myers", &patt_path],
        fasta,
        out
    );
    cmp_stdout!(
        &["find", "--to-csv", vars, "-d2", "--algo", "ukkonen", &patt_path],
        fasta,
        out
    );
}
