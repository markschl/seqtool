use super::*;

#[test]
fn pass() {
    Tester::new()
        .cmp(&["pass"], *FASTA, &FASTA)
        .cmp(&["."], *FASTA, &FASTA);
}

#[test]
fn append() {
    use std::fs::read_to_string;
    let fa = ">seq\nATGC\n";
    let t = Tester::new();
    t.temp_file("out.fa", None, |out_path, _| {
        let read_output = || read_to_string(out_path).unwrap();
        t.succeeds(&["pass", "--append", "-o", out_path], fa);
        assert_eq!(&read_output(), fa);
        t.succeeds(&["pass", "--append", "-o", out_path], fa);
        assert_eq!(&read_output(), &(fa.to_string() + fa));
        t.succeeds(&["pass", "--append", "-o", out_path], fa);
        assert_eq!(&read_output(), &(fa.to_string() + fa + fa));
    })
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
