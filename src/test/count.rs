use crate::helpers::NA;

use super::*;

#[test]
fn simple() {
    cmp(&["count"], *FASTA, "4\n");
    cmp(
        &["count", "-k", "attr(p)"],
        *FASTA,
        "1\t1\n10\t1\n11\t1\n2\t1\n",
    );
}

#[test]
fn fixed() {
    cmp(&["count"], *FASTA, "4\n");
    cmp(&["count"], *FASTA, "4\n");
    cmp(&["count", "-k", "num('2.3')"], *FASTA, "2.3\t4\n");
    cmp(&["count"], *FASTA, "4\n");
    cmp(&["count", "-k", "bin('2.3', 1)"], *FASTA, "(2, 3]\t4\n");
    cmp(
        &["count", "-k", "opt_attr(non_existent)"],
        *FASTA,
        &format!("{NA}\t4\n"),
    );
}

#[test]
fn multi() {
    let out = "25\t23\t1\n25\t24\t2\n25\t25\t1\n";

    cmp(&["count", "-k", "seqlen,ungapped_seqlen"], *FASTA, out);
    cmp(
        &["count", "-k", "seqlen", "-k", "ungapped_seqlen"],
        *FASTA,
        out,
    );
}

#[test]
fn discrete_bins() {
    cmp(
        &["count", "-k", "{bin(attr(p), 10)}"],
        *FASTA,
        "(0, 10]\t2\n(10, 20]\t2\n",
    );
}

const FLOAT_FASTA: &str = "\
>s1 a=1.10000000000002 =1.1
SEQ
>s2 a=0.00000000000001 =1e-14
SEQ
>s3 a=1.10000000000001 =1.1
SEQ
>s4 a=1.1000001 =1.1 with <=6 significant digits
SEQ
>s5 a=0.000000000000011 =1.1e-14
SEQ
>s6 a=11013452400000000001 =1.101345e19
SEQ
>s7 a=1.10000000000002 =1.1 (same as s1)
SEQ
";

#[test]
fn float() {
    cmp(
        &["count", "-k", "attr(a)"],
        FLOAT_FASTA,
        "0.00000000000001\t1\n0.000000000000011\t1\n1.10000000000001\t1\n1.10000000000002\t2\n1.1000001\t1\n11013452400000000001\t1\n",
    );
    cmp(
        &["count", "-k", "num(attr(a))"],
        FLOAT_FASTA,
        "1e-14\t1\n1.1e-14\t1\n1.10000\t4\n1.10135e19\t1\n",
    );
    cmp(
        &["count", "-k", "bin(attr(a), 1)"],
        FLOAT_FASTA,
        "(0, 1]\t2\n(1, 2]\t4\n(1.10135e19, 1.10135e19]\t1\n",
    );
}

#[test]
fn missing() {
    cmp(
        &["count", "-k", "{opt_attr(missing)}"],
        *FASTA,
        &format!("{NA}\t4\n"),
    );
    cmp(
        &["count", "-k", "{num(opt_attr(missing))}"],
        *FASTA,
        &format!("{NA}\t4\n"),
    );
    fails(
        &["count", "-k", "{attr(missing)}"],
        *FASTA,
        "'missing' not found in record",
    );

    #[cfg(feature = "expr")]
    {
        cmp(
            &["count", "-k", "{opt_attr('missing') + 1}"],
            *FASTA,
            "NaN\t4\n",
        );
        cmp(
            &["count", "-k", "{num(opt_attr('missing')) + 1}"],
            *FASTA,
            "NaN\t4\n",
        );
    }
}
