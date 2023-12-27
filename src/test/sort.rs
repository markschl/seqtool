use super::*;

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA (0) len=25, GC=0.6
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA (1) len=24, GC=0.667
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA (2) len=24, GC=0.667
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA (3) len=23, GC=0.652

#[test]
fn id_desc_seq() {
    Tester::new()
        .cmp(&["sort"], *FASTA, records!(3, 2, 1, 0))
        .cmp(&["sort", "-r"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["sort", "-k", "id"], *FASTA, records!(1, 0, 3, 2))
        .cmp(&["sort", "-k", "desc"], *FASTA, records!(1, 2, 3, 0))
        .cmp(&["sort", "-k", "{id} {desc}"], *FASTA, records!(1, 0, 3, 2));
}

#[test]
fn attr() {
    Tester::new()
        .cmp(&["sort", "-k", "attr(p)"], *FASTA, records!(1, 2, 3, 0))
        .cmp(&["sort", "-k", "{attr(p)}"], *FASTA, records!(1, 2, 3, 0));

    #[cfg(feature = "js")]
    Tester::new().cmp(&["sort", "-k", "{{attr(p)}}"], *FASTA, records!(1, 2, 3, 0));
}

#[test]
fn numeric_attr() {
    Tester::new()
        .cmp(&["sort", "-nk", "attr(p)"], *FASTA, records!(1, 0, 2, 3))
        .cmp(&["sort", "-nk", "{attr(p)}"], *FASTA, records!(1, 0, 2, 3))
        .cmp(&["sort", "-rnk", "attr(p)"], *FASTA, records!(3, 2, 0, 1));

    #[cfg(feature = "js")]
    Tester::new().cmp(
        &["sort", "-nk", "{{attr(p)+1}}"],
        *FASTA,
        records!(1, 0, 2, 3),
    );
}

#[test]
fn force_numeric() {
    Tester::new()
        .fails(&["sort", "-nk", "id"], *FASTA, "Could not convert")
        .fails(
            &["sort", "-nk", "{id}{attr(p)}"],
            *FASTA,
            "Could not convert",
        )
        .cmp(
            &["sort", "-nk", "{attr(p)}{attr(p)}"],
            *FASTA,
            records!(1, 0, 2, 3),
        );

    #[cfg(feature = "js")]
    Tester::new().cmp(
        &["sort", "-nk", "{{ id.substring(3, 4) }}"],
        *FASTA,
        records!(1, 0, 3, 2),
    );
}

#[test]
fn numeric_vars() {
    Tester::new()
        .cmp(&["sort", "-k", "num"], *FASTA, records!(0, 1, 2, 3))
        .cmp(&["sort", "-rk", "num"], *FASTA, records!(3, 2, 1, 0));

    #[cfg(feature = "js")]
    Tester::new()
        .cmp(
            &["sort", "-k", "{{ 7 + num }}"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        // num as string in range 1-4 -> same as numeric sort
        .cmp(
            &["sort", "-k", "{{ (num).toString() }}"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        // string sorting of: 8, 9, 10, 11 gives 10, 11, 8, 9
        .cmp(
            &["sort", "-k", "{{ (7 + num).toString() }}"],
            *FASTA,
            records!(2, 3, 0, 1),
        );

    Tester::new()
        .cmp(
            &["sort", "-k", "ungapped_seqlen"],
            *FASTA,
            records!(3, 1, 2, 0),
        )
        .cmp(
            &["sort", "-rk", "ungapped_seqlen"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(&["sort", "-k", "gc"], *FASTA, records!(0, 3, 1, 2))
        // -n argument has no effect (already numeric)
        .cmp(
            &["sort", "-rnk", "ungapped_seqlen"],
            *FASTA,
            records!(0, 1, 2, 3),
        )
        .cmp(&["sort", "-nk", "gc"], *FASTA, records!(0, 3, 1, 2));
}

#[test]
#[cfg(feature = "js")]
fn mixed_types() {
    Tester::new()
        // text before numeric
        .cmp(
            &[
                "sort",
                "-k",
                "{{ if (num <= 2) return num; else return 'text ' + num; }}",
            ],
            *FASTA,
            records!(2, 3, 0, 1),
        )
        // reverse order: numeric before text
        .cmp(
            &[
                "sort",
                "-rk",
                "{{ if (num <= 2) return num; else return 'text ' + num; }}",
            ],
            *FASTA,
            records!(1, 0, 3, 2),
        );
}

#[test]
#[cfg(feature = "js")]
fn key_var() {
    let fa = ">s1\nS1\n>s2\nS2\n>s3\nS3\n";
    let out = ">s3 k=-3\nS3\n>s1 k=\nS1\n>s2 k=\nS2\n";
    let expr = "{{ if (num <= 2) return undefined; return -parseInt(id.substring(1, 2)); }}";
    Tester::new()
        .cmp(&["sort", "-k", expr, "-a", "k={key}"], fa, out)
        .cmp(&["sort", "-nk", expr, "-a", "k={key}"], fa, out);
}
