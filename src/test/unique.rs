use indexmap::IndexSet;
use itertools::Itertools;
use rand::{seq::SliceRandom, SeedableRng};

use super::*;

#[test]
fn simple() {
    let t = Tester::new();
    t.cmp(&["unique", "seq"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "{seq}"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "seqhash"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "id"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "desc"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "{id} {desc}"], *FASTA, records!(0, 1, 2, 3));

    #[cfg(feature = "expr")]
    t.cmp(&["unique", "{seq + 'A'}"], *FASTA, records!(0, 1, 2, 3));
}

#[test]
fn attr() {
    let t = Tester::new();
    t.cmp(&["unique", "attr(p)"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "num(attr(p))"], *FASTA, records!(0, 1, 2, 3));

    #[cfg(feature = "expr")]
    t.cmp(&["unique", "{attr('p')+1}"], *FASTA, records!(0, 1, 2, 3));
}

#[test]
fn stats() {
    Tester::new()
        .cmp(&["unique", "seqlen"], *FASTA, records!(0))
        .cmp(&["unique", "num(seqlen)"], *FASTA, records!(0))
        .cmp(&["unique", "ungapped_seqlen"], *FASTA, records!(0, 1, 3))
        .cmp(&["unique", "gc"], *FASTA, records!(0, 1, 3));
}

#[test]
fn numeric() {
    let t = Tester::new();
    t.fails(&["unique", "num(id)"], *FASTA, "Could not convert");
    #[cfg(feature = "expr")]
    t.fails(
        &["unique", "{num(id + attr('p'))}"],
        *FASTA,
        "Could not convert",
    )
    .cmp(
        &["unique", "{num(attr('p') + attr('p'))}"],
        *FASTA,
        records!(0, 1, 2, 3),
    )
    .fails(&["unique", "{num(id)}"], *FASTA, "Could not convert")
    .cmp(
        &["unique", "{ num(id.substring(3, 4)) }"],
        *FASTA,
        records!(0, 1, 2, 3),
    );
}

#[test]
#[cfg(feature = "expr")]
fn expr() {
    Tester::new()
        .cmp(
            &["unique", "{ seq_num + parseInt(attr('p')) }"],
            *FASTA,
            records!(0, 2, 3),
        )
        .cmp(
            &[
                "unique",
                "{ if (seq_num <= 2) return seq_num; return (seq_num).toString(); }",
            ],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(
            &[
                "unique",
                "{ if (seq_num <= 2) return seq_num; return undefined; }",
            ],
            *FASTA,
            records!(0, 1, 2),
        );
}

#[test]
fn multi_key() {
    let parts = &[">s1 1\nA\n", ">s2 1\nA\n", ">s3 2\nA\n", ">s4 1\nB\n"];
    let fa = parts.join("");
    macro_rules! sel {
        ($($i:expr),*) => {
            &[$($i),*].into_iter().map(|i| &parts[i]).join("")
        }
    }
    let t = Tester::new();
    t.cmp(&["unique", "desc"], &fa, sel!(0, 2))
        .cmp(&["unique", "desc,seq"], &fa, sel!(0, 2, 3))
        .cmp(&["unique", "id,desc,seq"], &fa, sel!(0, 1, 2, 3))
        .cmp(&["unique", "seq"], &fa, sel!(0, 3))
        .cmp(
            &["unique", "desc,seq", "-a", "k={key}"],
            &fa,
            ">s1 1 k=1,A\nA\n>s3 2 k=2,A\nA\n>s4 1 k=1,B\nB\n",
        );

    #[cfg(feature = "expr")]
    t.cmp(&["unique", "{desc + 1},{seq.length}"], &fa, sel!(0, 2));
}

#[test]
fn hash() {
    let fa = ">s1\nAGGCUG\n>s2\nCAGCCU\n";
    Tester::new()
        .cmp(&["unique", "seqhash", "--to-tsv", "id"], fa, "s1\ns2\n")
        .cmp(&["unique", "seqhash_rev", "--to-tsv", "id"], fa, "s1\ns2\n")
        .cmp(&["unique", "seqhash_both", "--to-tsv", "id"], fa, "s1\n")
        // 'U' not reverse complemented
        .cmp(
            &[
                "unique",
                "seqhash_both",
                "--seqtype",
                "dna",
                "--to-tsv",
                "id",
            ],
            fa,
            "s1\ns2\n",
        );
}

#[test]
fn case() {
    let fa = ">s1\nAg\n>s2\naG\n>s3\nCt\n";
    Tester::new()
        .cmp(
            &["unique", "seqhash(false)", "--to-tsv", "id"],
            fa,
            "s1\ns2\ns3\n",
        )
        .cmp(
            &["unique", "seqhash(true)", "--to-tsv", "id"],
            fa,
            "s1\ns3\n",
        )
        .cmp(&["unique", "upper_seq", "--to-tsv", "id"], fa, "s1\ns3\n")
        .cmp(&["unique", "lower_seq", "--to-tsv", "id"], fa, "s1\ns3\n")
        .cmp(
            &["unique", "seqhash_both(true)", "--to-tsv", "id"],
            fa,
            "s1\n",
        );
}

#[test]
#[cfg(feature = "expr")]
fn key_var() {
    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = ">s1 k=-1\nS1\n>s2 k=\nS2\n";
    let formula = "{ if (seq_num <= 1) return -parseInt(id.substring(1, 2)); return undefined; }";
    Tester::new().cmp(&["unique", formula, "-a", "k={key}"], fa, out);
}

#[test]
fn dup_var() {
    let fa = ">s1\nA\n>s2\nA\n>s3\nB\n";
    let dup_out = ">s1 n=2\nA\n>s3 n=1\nB\n";
    let dup_out0 = ">s1 n=1\nA\n>s3 n=0\nB\n";
    let ids_out = ">s1 l=s1,s2\nA\n>s3 l=s3\nB\n";
    let ids_out0 = ">s1 l=s2\nA\n>s3 l=\nB\n";
    Tester::new()
        .cmp(&["unique", "seq", "-a", "n={n_duplicates}"], fa, dup_out)
        .cmp(
            &["unique", "seq", "-a", "n={n_duplicates}", "-M", "0", "-q"],
            fa,
            dup_out,
        )
        .cmp(
            &["unique", "seq", "-a", "n={n_duplicates(false)}"],
            fa,
            dup_out0,
        )
        .cmp(&["unique", "seq", "-a", "l={duplicates_list}"], fa, ids_out)
        .cmp(
            &[
                "unique",
                "seq",
                "-a",
                "l={duplicates_list}",
                "-M",
                "0",
                "-q",
            ],
            fa,
            ids_out,
        )
        .cmp(
            &["unique", "seq", "-a", "l={duplicates_list(false)}"],
            fa,
            ids_out0,
        );
}

/// Tests larger input and different memory limits
#[test]
fn large() {
    // the expected output is a collection of 100 records
    let n_records = 100;
    let unique_idx_sorted: Vec<_> = (0..n_records).collect();
    // create 4 duplicates per item
    let mut all_idx = unique_idx_sorted.clone();
    for _ in 0..4 {
        all_idx.extend_from_slice(&unique_idx_sorted);
    }
    // shuffle the result
    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(42);
    all_idx.shuffle(&mut rng);
    // then, we obtain unique records in order of occurrence
    let mut idx_map: IndexSet<_> = all_idx.iter().cloned().collect();
    let unique_idx_inorder: Vec<_> = idx_map.drain(..).collect();

    // get the formatted FASTA records
    let unique_rec_sorted: Vec<_> = unique_idx_sorted
        .iter()
        .map(|i| format!(">{}\nSEQ\n", i))
        .collect();
    let unique_fasta_sorted = unique_rec_sorted.join("");
    let all_fasta = all_idx
        .iter()
        .map(|i| unique_rec_sorted[*i].clone())
        .join("");
    let unique_fasta_inorder = unique_idx_inorder
        .iter()
        .map(|i| unique_rec_sorted[*i].clone())
        .join("");

    let t = Tester::new();
    t.temp_file("unique", Some(&all_fasta), |path, _| {
        // below memory limit: output in order of input
        t.cmp(&["unique", "id"], FileInput(path), &unique_fasta_inorder)
            .cmp(
                &["unique", "num(id)"],
                FileInput(path),
                &unique_fasta_inorder,
            )
            // ...unless --sort is supplied
            .cmp(
                &["unique", "num(id)", "--sort"],
                FileInput(path),
                &unique_fasta_sorted,
            );
        // with memory limit: adding --sort, since otherwise the sort order
        // is not guaranteed
        // (numeric sort in this case)
        for rec_limit in [1, 5, 10, 20, 30, 50, 60, 80, 100, 120] {
            // A full record with a 2-digit ID should currently use 82 bytes
            // (in sorting mode)
            let sort_limit = format!("{}", rec_limit * 82);
            t.cmp(
                &["unique", "num(id)", "-M", &sort_limit, "-s", "-q"],
                FileInput(path),
                &unique_fasta_sorted,
            );
            // Unsorted dereplication can only be tested if additionally sorting the output,
            // since there is no stable output order with a memory limit.
            // The record key should be ~26 bytes.
            let simple_limit = format!("{}", rec_limit * 26);
            #[cfg(any(feature = "all-commands", feature = "sort"))]
            t.pipe(
                &["unique", "num(id)", "-M", &simple_limit, "-q"],
                &all_fasta,
                &["sort", "num(id)"],
                &unique_fasta_sorted,
            );
        }
    });
}

#[test]
fn map_out() {
    use std::io::read_to_string;
    let fa = ">s1 a=1\nSEQ\n>s2 a=1\nSEQ\n>s3 a=2\nSEQ\n>s4 a=1\nSEQ\n";
    let unique_fa = ">s1 a=1\nSEQ\n>s3 a=2\nSEQ\n";
    let long = "s1\ts1\ns2\ts1\ns4\ts1\ns3\ts3\n";
    let long_star = "s1\t*\ns2\ts1\ns4\ts1\ns3\t*\n";
    let wide = "s1\ts2\ts4\ns3\n";
    let wide_comma = "s1\ts1,s2,s4\ns3\ts3\n";
    let wide_key = "1\ts1\ts2\ts4\n2\ts3\n";
    let t = Tester::new();
    t.temp_dir("find_drop", |d| {
        let out = d.path().join("map.tsv");
        let out_path = out.to_str().unwrap();
        let read_file = || read_to_string(File::open(out_path).unwrap()).unwrap();
        t.cmp(&["unique", "attr(a)", "--map-out", out_path], fa, unique_fa);
        assert_eq!(&read_file(), long);
        t.cmp(
            &[
                "unique",
                "attr(a)",
                "--map-out",
                out_path,
                "--map-fmt",
                "long-star",
            ],
            fa,
            unique_fa,
        );
        assert_eq!(&read_file(), long_star);
        t.cmp(
            &[
                "unique",
                "attr(a)",
                "--map-out",
                out_path,
                "--map-fmt",
                "wide",
            ],
            fa,
            unique_fa,
        );
        assert_eq!(&read_file(), wide);
        t.cmp(
            &[
                "unique",
                "attr(a)",
                "--map-out",
                out_path,
                "--map-fmt",
                "wide-comma",
            ],
            fa,
            unique_fa,
        );
        assert_eq!(&read_file(), wide_comma);
        t.cmp(
            &[
                "unique",
                "attr(a)",
                "--map-out",
                out_path,
                "--map-fmt",
                "wide-key",
            ],
            fa,
            unique_fa,
        );
        assert_eq!(&read_file(), wide_key);
    })
}
