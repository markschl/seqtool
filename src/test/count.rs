
use super::*;


#[test]
fn count() {
    Tester::new()
        .cmp(&["count"], *FASTA, "4\n")
        .cmp(&["count", "-k", "a:p"], *FASTA, "1\t1\n10\t1\n11\t1\n2\t1\n");
}

#[test]
fn numeric() {
    Tester::new()
        .cmp(&["count", "-k", "n:10:{a:p}"], *FASTA, "(0,10]\t2\n(10,20]\t2\n")
        .cmp(&["count", "-nk", "n:10:{a:p}"], *FASTA, "0\t2\n10\t2\n");
}

#[test]
fn missing() {
    Tester::new()
        .cmp(&["count", "-k", "{a:missing}"], *FASTA, "\t4\n")
        .cmp(&["count", "-k", "n:{a:missing}"], *FASTA, "N/A\t4\n")
        .cmp(&["count", "-k", "{{a:missing + 1}}"], *FASTA, "NaN\t4\n")
        .cmp(&["count", "-k", "n:{{a:missing + 1}}"], *FASTA, "NaN\t4\n");
}
