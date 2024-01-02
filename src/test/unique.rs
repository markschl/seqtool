use indexmap::IndexSet;
use itertools::Itertools;
use rand::{seq::SliceRandom, SeedableRng};

use super::*;

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA (0) len=25, GC=0.6
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA (1) len=24, GC=0.667
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA (2) len=24, GC=0.667
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA (3) len=23, GC=0.652

#[test]
fn simple() {
    Tester::new()
        .cmp(&["unique"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "seq"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "{seq}"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "id"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "desc"], *FASTA, records!(0, 1, 2, 3))
        .cmp(
            &["unique", "-k", "{id} {desc}"],
            *FASTA,
            records!(0, 1, 2, 3),
        );

    #[cfg(feature = "expr")]
    Tester::new().cmp(&["unique", "-k", "{{seq}}"], *FASTA, records!(0, 1, 2, 3));
}

#[test]
fn attr() {
    Tester::new()
        .cmp(&["unique", "-k", "attr(p)"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-nk", "attr(p)"], *FASTA, records!(0, 1, 2, 3));

    #[cfg(feature = "expr")]
    Tester::new().cmp(
        &["unique", "-k", "{{attr(p)+1}}"],
        *FASTA,
        records!(0, 1, 2, 3),
    );
}

#[test]
fn stats() {
    Tester::new()
        .cmp(&["unique", "-k", "seqlen"], *FASTA, records!(0))
        .cmp(&["unique", "-nk", "seqlen"], *FASTA, records!(0))
        .cmp(
            &["unique", "-k", "ungapped_seqlen"],
            *FASTA,
            records!(0, 1, 3),
        )
        .cmp(&["unique", "-k", "gc"], *FASTA, records!(0, 1, 3));
}

#[test]
fn force_numeric() {
    Tester::new()
        .fails(&["unique", "-nk", "id"], *FASTA, "Could not convert")
        .fails(
            &["unique", "-nk", "{id}{attr(p)}"],
            *FASTA,
            "Could not convert",
        )
        .cmp(
            &["unique", "-nk", "{attr(p)}{attr(p)}"],
            *FASTA,
            records!(0, 1, 2, 3),
        );

    #[cfg(feature = "expr")]
    Tester::new()
        .fails(&["unique", "-nk", "{{id}}"], *FASTA, "Could not convert")
        .cmp(
            &["unique", "-nk", "{{ id.substring(3, 4) }}"],
            *FASTA,
            records!(0, 1, 2, 3),
        );
}

#[test]
#[cfg(feature = "expr")]
fn expr() {
    Tester::new()
        .cmp(
            &["unique", "-k", "{{ num + parseInt(attr(p)) }}"],
            *FASTA,
            records!(0, 2, 3),
        )
        .cmp(
            &[
                "unique",
                "-k",
                "{{ if (num <= 2) return num; return (num).toString(); }}",
            ],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(
            &[
                "unique",
                "-k",
                "{{ if (num <= 2) return num; return undefined; }}",
            ],
            *FASTA,
            records!(0, 1, 2),
        );
}

#[test]
#[cfg(feature = "expr")]
fn key_var() {
    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = ">s1 k=-1\nS1\n>s2 k=\nS2\n";
    let formula = "{{ if (num <= 1) return -parseInt(id.substring(1, 2)); return undefined; }}";
    Tester::new()
        .cmp(&["unique", "-k", formula, "-a", "k={key}"], fa, out)
        .cmp(&["unique", "-nk", formula, "-a", "k={key}"], fa, out);
}

#[test]
fn large() {
    // the expected output is a collection of 100 records
    let n_records = 100;
    let unique_idx_sorted: Vec<_> = (0..n_records).collect();
    // create some duplicates
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
        // without memory limit: output in order of input
        t.cmp(
            &["unique", "-k", "id"],
            FileInput(path),
            &unique_fasta_inorder,
        )
        .cmp(
            &["unique", "-k", "id", "-n"],
            FileInput(path),
            &unique_fasta_inorder,
        )
        // ...unless --sort is supplied
        .cmp(
            &["unique", "-k", "id", "--sort", "-n"],
            FileInput(path),
            &unique_fasta_sorted,
        );
        // with memory limit: should always be sorted
        // (numeric sort in this case)
        for rec_limit in [5, 10, 20, 50, 80] {
            // a record with a 3-digit ID should have 66 bytes
            // (ID key: 3, formatted record: 9, Vec sizes: 24 + 32)
            let mem_limit = rec_limit * 68;
            let mem = format!("{}", mem_limit);
            t.cmp(
                &["unique", "-k", "id", "-n", "--max-mem", &mem],
                FileInput(path),
                &unique_fasta_sorted,
            )
            .cmp(
                &["unique", "-k", "id", "-n", "--max-mem", &mem, "--sort"],
                FileInput(path),
                &unique_fasta_sorted,
            );
        }
    });
}
