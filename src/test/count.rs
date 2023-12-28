use super::*;

#[test]
fn count() {
    Tester::new().cmp(&["count"], *FASTA, "4\n").cmp(
        &["count", "-k", "attr(p)"],
        *FASTA,
        "1\t1\n10\t1\n11\t1\n2\t1\n",
    );
}

#[test]
fn numeric() {
    Tester::new()
        .cmp(
            &["count", "-k", "n:10:{attr(p)}"],
            *FASTA,
            "(0,10]\t2\n(10,20]\t2\n",
        )
        .cmp(&["count", "-nk", "n:10:{attr(p)}"], *FASTA, "0\t2\n10\t2\n");
}

#[test]
fn missing() {
    Tester::new()
        .cmp(&["count", "-k", "{opt_attr(missing)}"], *FASTA, "N/A\t4\n")
        .cmp(
            &["count", "-k", "n:{opt_attr(missing)}"],
            *FASTA,
            "N/A\t4\n",
        )
        .fails(
            &["count", "-k", "{attr(missing)}"],
            *FASTA,
            "'missing' not found in record",
        );

    #[cfg(feature = "expr")]
    Tester::new()
        .cmp(
            &["count", "-k", "{{opt_attr(missing) + 1}}"],
            *FASTA,
            "NaN\t4\n",
        )
        .cmp(
            &["count", "-k", "n:{{opt_attr(missing) + 1}}"],
            *FASTA,
            "NaN\t4\n",
        );
}
