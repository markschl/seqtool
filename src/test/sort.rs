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
    cmp(&["sort", "seq"], *FASTA, records!(3, 2, 1, 0));
    cmp(&["sort", "-r", "seq"], *FASTA, records!(0, 1, 2, 3));
    cmp(&["sort", "id"], *FASTA, records!(1, 0, 3, 2));
    cmp(&["sort", "desc"], *FASTA, records!(1, 2, 3, 0));
    cmp(&["sort", "{id}_{desc}"], *FASTA, records!(1, 0, 3, 2));
}

#[test]
fn attr() {
    cmp(&["sort", "attr(p)"], *FASTA, records!(1, 2, 3, 0));
    cmp(&["sort", "{attr(p)}"], *FASTA, records!(1, 2, 3, 0));
}

#[test]
fn numeric_attr() {
    cmp(&["sort", "num(attr(p))"], *FASTA, records!(1, 0, 2, 3));
    cmp(&["sort", "{num(attr(p))}"], *FASTA, records!(1, 0, 2, 3));
    cmp(
        &["sort", "-r", "num(attr(p))"],
        *FASTA,
        records!(3, 2, 0, 1),
    );
    cmp(
        &["sort", "-r", "{num(attr('p'))}"],
        *FASTA,
        records!(3, 2, 0, 1),
    );

    #[cfg(feature = "expr")]
    cmp(
        &["sort", "{num(attr('p')+1)}"],
        *FASTA,
        records!(1, 0, 2, 3),
    );
}

#[test]
fn numeric() {
    fails(&["sort", "num(id)"], *FASTA, "Could not convert");
    #[cfg(feature = "expr")]
    {
        fails(
            &["sort", "{num(id + attr('p'))}"],
            *FASTA,
            "Could not convert",
        );
        cmp(
            &["sort", "{num(attr('p') + attr('p'))}"],
            *FASTA,
            records!(1, 0, 2, 3),
        );
        cmp(
            &["sort", "{ num(id.substring(3, 4)) }"],
            *FASTA,
            records!(1, 0, 3, 2),
        );
    }
}

#[test]
fn numeric_vars() {
    cmp(&["sort", "seq_num"], *FASTA, records!(0, 1, 2, 3));
    cmp(&["sort", "-r", "seq_num"], *FASTA, records!(3, 2, 1, 0));

    #[cfg(feature = "expr")]
    {
        cmp(&["sort", "{ 7 + seq_num }"], *FASTA, records!(0, 1, 2, 3));
        // seq_num as string in range 1-4 -> same as numeric sort
        cmp(
            &["sort", "{ (seq_num).toString() }"],
            *FASTA,
            records!(0, 1, 2, 3),
        );
        // string sorting of: 8, 9, 10, 11 gives 10, 11, 8, 9
        cmp(
            &["sort", "{ (7 + seq_num).toString() }"],
            *FASTA,
            records!(2, 3, 0, 1),
        );
    }

    cmp(&["sort", "ungapped_seqlen"], *FASTA, records!(3, 1, 2, 0));
    cmp(
        &["sort", "-r", "ungapped_seqlen"],
        *FASTA,
        records!(0, 1, 2, 3),
    );
    cmp(&["sort", "gc"], *FASTA, records!(0, 3, 1, 2));
    // -n argument has no effect (already numeric);
    cmp(
        &["sort", "-r", "num(ungapped_seqlen)"],
        *FASTA,
        records!(0, 1, 2, 3),
    );
    cmp(&["sort", "num(gc)"], *FASTA, records!(0, 3, 1, 2));
}

#[test]
fn multi_key() {
    cmp(
        &["sort", "-r", "num(gc),num(attr('p'))"],
        *FASTA,
        records!(2, 1, 3, 0),
    );
    #[cfg(feature = "expr")]
    cmp(
        &["sort", "seqlen,ungapped_seqlen,{-attr('p')}"],
        *FASTA,
        records!(3, 2, 1, 0),
    );
}

#[test]
#[cfg(feature = "expr")]
fn mixed_types() {
    // text before numeric
    cmp(
        &[
            "sort",
            "{ if (seq_num <= 2) return seq_num; else return 'text ' + seq_num; }",
        ],
        *FASTA,
        records!(2, 3, 0, 1),
    );
    // reverse order: numeric before text
    cmp(
        &[
            "sort",
            "-r",
            "{ if (seq_num <= 2) return seq_num; else return 'text ' + seq_num; }",
        ],
        *FASTA,
        records!(1, 0, 3, 2),
    );
}

#[test]
#[cfg(feature = "expr")]
fn key_var() {
    use crate::helpers::NA;

    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = &format!(">s3 k=-3\nS3\n>s1 k={NA}\nS1\n>s2 k={NA}\nS2\n");
    let expr = "{ if (seq_num <= 2) return undefined; return -parseInt(id.substring(1, 2)); }";
    cmp(&["sort", expr, "-a", "k={key}"], fa, out);
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
        .map(|(i, idx)| (idx, format!(">{idx} {i}\nSEQ\n")))
        .collect();
    let mut text_sorted = seqs.clone();
    text_sorted.sort_by_key(|(i, _)| format!("{i}"));
    let mut rev_sorted = seqs.clone();
    rev_sorted.sort_by_key(|(i, _)| Reverse(format!("{i}")));
    let mut num_sorted = seqs.clone();
    num_sorted.sort_by_key(|(i, _)| *i);
    let fasta = seqs.iter().map(|(_, s)| s).join("");
    let sorted_fasta = text_sorted.iter().map(|(_, s)| s).join("");
    let rev_sorted_fasta = rev_sorted.iter().map(|(_, s)| s).join("");
    let num_sorted_fasta = num_sorted.iter().map(|(_, s)| s).join("");

    let input = tmp_file("st_sort_large_", ".fasta", &fasta);
    for rec_limit in [5usize, 10, 20, 50, 100, 150, 1000] {
        // a record with a 2-digit ID should currently occupy 50 bytes (text);
        // or 48 bytes (numeric);
        let text_limit = format!("{}", rec_limit * 50);
        let num_limit = format!("{}", rec_limit * 48);
        cmp(
            &["sort", "id", "-M", &text_limit, "-q"],
            &input,
            &sorted_fasta,
        );
        cmp(
            &["sort", "-r", "id", "-M", &text_limit, "-q"],
            &input,
            &rev_sorted_fasta,
        );
        cmp(
            &["sort", "num(id)", "-M", &num_limit, "-q"],
            &input,
            &num_sorted_fasta,
        );
    }
}
