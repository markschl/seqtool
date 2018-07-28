
use std::str;

use super::*;

#[test]
fn stats() {
    let seq = ">seq\nATGC-NYA\n";
    let retval = "seq\t8\t7\t40\t2\t3";
    let vars = "s:seqlen,s:ungapped_len,s:gc,s:count:A,s:count:AT";
    let vars_noprefix = vars.replace("s:", "");
    let retval2 = format!("id\t{}\n{}", vars_noprefix.replace(",", "\t"), retval);
    Tester::new()
        .cmp(&[".", "--to-tsv", &format!("id,{}", vars)], seq, retval)
        .cmp(&["stat", &vars_noprefix], seq, &retval2);
}

#[test]
fn qualstat() {
    Tester::new()
        .cmp(
            &[".", "--fq", "--to-tsv", "s:exp_err"],
            &format!("@id\nAAA\n+\n{}\n", str::from_utf8(&[33, 43, 53]).unwrap()), "1.11\n"
        )
        .cmp(
            &[".", "--fq-illumina", "--to-tsv", "s:exp_err"],
            &format!("@id\nAAA\n+\n{}\n", str::from_utf8(&[64, 74, 84]).unwrap()), "1.11\n"
        )
        .fails(
            &[".", "--fq", "--to-tsv", "s:exp_err"],
            &format!("@id\nA\n+\n{}\n", str::from_utf8(&[32]).unwrap()), "Invalid quality"
        )
        .fails(&[".", "--to-tsv", "s:exp_err"], ">seq\nAA", "No quality scores");
}
