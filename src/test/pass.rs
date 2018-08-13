use super::*;

#[test]
fn pass() {
    Tester::new()
        .cmp(&["pass"], *FASTA, &FASTA)
        .cmp(&["."], *FASTA, &FASTA);
}

#[test]
fn fasta_io() {
    let fa = ">seq\nATGC\n";
    let fa_wrap = ">seq\nAT\nGC\n";
    let fa_wrap3 = ">seq\nATG\nC\n";
    Tester::new()
        .cmp(&["."], fa, fa)
        .cmp(&["."], fa_wrap, fa)
        .cmp(&[".", "--wrap", "2"], fa, fa_wrap)
        .cmp(&[".", "--wrap", "3"], fa_wrap, fa_wrap3);
}

#[test]
fn pipe() {
    Tester::new().pipe(&["."], &FASTA, &["."], &FASTA);
}

#[test]
fn thread_io() {
    Tester::new().cmp(&[".", "-T", "--write-thread"], *FASTA, &FASTA);
}
