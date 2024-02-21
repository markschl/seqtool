use std::str;

use super::*;

#[test]
fn stats() {
    let seq = ">seq\nATGC-NYA\n";
    let retval = "seq\t8\t7\t40\t2\t3\n";
    let vars = "seqlen,ungapped_seqlen,gc,charcount(A),charcount(AT)";
    #[cfg(feature = "pass")]
    Tester::new().cmp(&[".", "--to-tsv", &format!("id,{}", vars)], seq, retval);
    Tester::new().cmp(&["stat", vars], seq, retval);
}

#[test]
fn qualstat() {
    Tester::new()
        .cmp(
            &["stat", "--fq", "exp_err"],
            &format!("@id\nAAA\n+\n{}\n", str::from_utf8(&[33, 43, 53]).unwrap()),
            "id\t1.11\n",
        )
        .cmp(
            &["stat", "--fq-illumina", "exp_err"],
            &format!("@id\nAAA\n+\n{}\n", str::from_utf8(&[64, 74, 84]).unwrap()),
            "id\t1.11\n",
        )
        .fails(
            &["stat", "--fq", "exp_err"],
            &format!("@id\nA\n+\n{}\n", str::from_utf8(&[32]).unwrap()),
            "Invalid quality",
        )
        .fails(&["stat", "exp_err"], ">seq\nAA", "No quality scores");
}
