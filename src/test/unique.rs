use itertools::Itertools;

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
        .cmp(&["unique", "-k", "{{seq}}"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "id"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-k", "desc"], *FASTA, records!(0, 1, 2, 3))
        .cmp(
            &["unique", "-k", "{id} {desc}"],
            *FASTA,
            records!(0, 1, 2, 3),
        );
}

#[test]
fn attr() {
    Tester::new()
        .cmp(&["unique", "-k", "attr(p)"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["unique", "-nk", "attr(p)"], *FASTA, records!(0, 1, 2, 3))
        .cmp(
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
        .fails(&["unique", "-nk", "{{id}}"], *FASTA, "Could not convert")
        .fails(
            &["unique", "-nk", "{id}{attr(p)}"],
            *FASTA,
            "Could not convert",
        )
        .cmp(
            &["unique", "-nk", "{attr(p)}{attr(p)}"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(
            &["unique", "-nk", "{{ id.substring(3, 4) }}"],
            *FASTA,
            records!(0, 1, 2, 3),
        );
}

#[test]
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
fn key_var() {
    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = ">s1 k=-1\nS1\n>s2 k=\nS2\n";
    let formula = "{{ if (num <= 1) return -parseInt(id.substring(1, 2)); return undefined; }}";
    Tester::new()
        .cmp(&["unique", "-k", formula, "-a", "k={key}"], fa, out)
        .cmp(&["unique", "-nk", formula, "-a", "k={key}"], fa, out);
}
