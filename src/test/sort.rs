use std::cmp::Reverse;

use rand::{seq::SliceRandom, SeedableRng};

use super::*;

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA (0) len=25, GC=0.6
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA (1) len=24, GC=0.667
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA (2) len=24, GC=0.667
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA (3) len=23, GC=0.652

#[test]
fn id_desc_seq() {
    Tester::new()
        .cmp(&["sort", "seq"], *FASTA, records!(3, 2, 1, 0))
        .cmp(&["sort", "-r", "seq"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["sort", "id"], *FASTA, records!(1, 0, 3, 2))
        .cmp(&["sort", "desc"], *FASTA, records!(1, 2, 3, 0))
        .cmp(&["sort", "{id}_{desc}"], *FASTA, records!(1, 0, 3, 2));
}

#[test]
fn attr() {
    Tester::new()
        .cmp(&["sort", "attr(p)"], *FASTA, records!(1, 2, 3, 0))
        .cmp(&["sort", "{attr(p)}"], *FASTA, records!(1, 2, 3, 0));
}

#[test]
fn numeric_attr() {
    Tester::new()
        .cmp(&["sort", "-n", "attr(p)"], *FASTA, records!(1, 0, 2, 3))
        .cmp(&["sort", "-n", "{attr(p)}"], *FASTA, records!(1, 0, 2, 3))
        .cmp(&["sort", "-rn", "attr(p)"], *FASTA, records!(3, 2, 0, 1));

    #[cfg(feature = "expr")]
    Tester::new().cmp(
        &["sort", "-n", "{attr('p')+1}"],
        *FASTA,
        records!(1, 0, 2, 3),
    );
}

#[test]
fn force_numeric() {
    Tester::new()
        .fails(&["sort", "-n", "id"], *FASTA, "Could not convert")
        .fails(
            &["sort", "-n", "{id}{attr(p)}"],
            *FASTA,
            "Could not convert",
        )
        .cmp(
            &["sort", "-n", "{attr(p)}{attr(p)}"],
            *FASTA,
            records!(1, 0, 2, 3),
        );

    #[cfg(feature = "expr")]
    Tester::new().cmp(
        &["sort", "-n", "{ id.substring(3, 4) }"],
        *FASTA,
        records!(1, 0, 3, 2),
    );
}

#[test]
fn numeric_vars() {
    Tester::new()
        .cmp(&["sort", "num"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["sort", "-r", "num"], *FASTA, records!(3, 2, 1, 0));

    #[cfg(feature = "expr")]
    Tester::new()
        .cmp(&["sort", "{ 7 + num }"], *FASTA, records!(0, 1, 2, 3))
        // num as string in range 1-4 -> same as numeric sort
        .cmp(
            &["sort", "{ (num).toString() }"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        // string sorting of: 8, 9, 10, 11 gives 10, 11, 8, 9
        .cmp(
            &["sort", "{ (7 + num).toString() }"],
            *FASTA,
            records!(2, 3, 0, 1),
        );

    Tester::new()
        .cmp(&["sort", "ungapped_seqlen"], *FASTA, records!(3, 1, 2, 0))
        .cmp(
            &["sort", "-r", "ungapped_seqlen"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(&["sort", "gc"], *FASTA, records!(0, 3, 1, 2))
        // -n argument has no effect (already numeric)
        .cmp(
            &["sort", "-rn", "ungapped_seqlen"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(&["sort", "-n", "gc"], *FASTA, records!(0, 3, 1, 2));
}

#[test]
fn multi_key() {
    Tester::new().cmp(&["sort", "-rn", "gc,attr(p)"], *FASTA, records!(2, 1, 3, 0));
    #[cfg(feature = "expr")]
    Tester::new().cmp(
        &["sort", "seqlen,ungapped_seqlen,{-attr('p')}"],
        *FASTA,
        records!(3, 2, 1, 0),
    );
}

#[test]
#[cfg(feature = "expr")]
fn mixed_types() {
    Tester::new()
        // text before numeric
        .cmp(
            &[
                "sort",
                "{ if (num <= 2) return num; else return 'text ' + num; }",
            ],
            *FASTA,
            records!(2, 3, 0, 1),
        )
        // reverse order: numeric before text
        .cmp(
            &[
                "sort",
                "-r",
                "{ if (num <= 2) return num; else return 'text ' + num; }",
            ],
            *FASTA,
            records!(1, 0, 3, 2),
        );
}

#[test]
#[cfg(feature = "expr")]
fn key_var() {
    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = ">s3 k=-3\nS3\n>s1 k=\nS1\n>s2 k=\nS2\n";
    let expr = "{ if (num <= 2) return undefined; return -parseInt(id.substring(1, 2)); }";
    Tester::new()
        .cmp(&["sort", expr, "-a", "k={key}"], fa, out)
        .cmp(&["sort", "-n", expr, "-a", "k={key}"], fa, out);
}

#[test]
fn large() {
    // Randomly shuffle records (with sequence number in ID),
    // in order to later sort them by ID.
    // Each ID is repeated twice, so we can test handling of ties at different
    // memory limits.
    let n_records = 200;
    let mut indices: Vec<_> = (0usize..n_records / 2).chain(0..n_records / 2).collect();
    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(42);
    indices.shuffle(&mut rng);
    let seqs: Vec<_> = indices
        .into_iter()
        .enumerate()
        .map(|(i, idx)| (idx, format!(">{} {}\nSEQ\n", idx, i)))
        .collect();
    let mut text_sorted = seqs.clone();
    text_sorted.sort_by_key(|(i, _)| format!("{}", i));
    let mut rev_sorted = seqs.clone();
    rev_sorted.sort_by_key(|(i, _)| Reverse(format!("{}", i)));
    let mut num_sorted = seqs.clone();
    num_sorted.sort_by_key(|(i, _)| *i);
    let fasta = seqs.iter().map(|(_, s)| s).join("");
    let sorted_fasta = text_sorted.iter().map(|(_, s)| s).join("");
    let rev_sorted_fasta = rev_sorted.iter().map(|(_, s)| s).join("");
    let num_sorted_fasta = num_sorted.iter().map(|(_, s)| s).join("");

    let t = Tester::new();
    t.temp_file("sort", Some(&fasta), |path, _| {
        for rec_limit in [5usize, 10, 20, 50, 100, 150, 1000] {
            // a record with a 2-digit ID should currently occupy 50 bytes (text)
            // or 48 bytes (numeric)
            let text_limit = format!("{}", rec_limit * 50);
            let num_limit = format!("{}", rec_limit * 48);
            t.cmp(
                &["sort", "id", "-M", &text_limit, "-q"],
                FileInput(path),
                &sorted_fasta,
            );
            t.cmp(
                &["sort", "-r", "id", "-M", &text_limit, "-q"],
                FileInput(path),
                &rev_sorted_fasta,
            );
            t.cmp(
                &["sort", "-n", "id", "-M", &num_limit, "-q"],
                FileInput(path),
                &num_sorted_fasta,
            );
        }
    });
}
