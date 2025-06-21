use super::*;

#[test]
fn pass() {
    cmp(&["pass"], *FASTA, &FASTA);
    cmp(&["."], *FASTA, &FASTA);
}

#[test]
fn append() {
    with_tmpdir("st_pass_append_", |td| {
        let fa = ">seq\nATGC\n";
        let out = td.path("pass_append_out.fasta");
        succeeds(&["pass", "--append", "-o", &out], fa);
        assert_eq!(&out.content(), fa);
        succeeds(&["pass", "--append", "-o", &out], fa);
        assert_eq!(&out.content(), &(fa.to_string() + fa));
        succeeds(&["pass", "--append", "-o", &out], fa);
        assert_eq!(&out.content(), &(fa.to_string() + fa + fa));
    });
}

#[test]
fn fasta_io() {
    let fa = ">seq\nATGC\n";
    let fa_wrap = ">seq\nAT\nGC\n";
    let fa_wrap3 = ">seq\nATG\nC\n";

    cmp(&["."], fa, fa);
    cmp(&["."], fa_wrap, fa);
    cmp(&[".", "--wrap", "2"], fa, fa_wrap);
    cmp(&[".", "--wrap", "3"], fa_wrap, fa_wrap3);
}

#[test]
fn pass_pipe() {
    cmp_pipe(&["."], &FASTA, &["."], &FASTA);
}

#[test]
fn thread_io() {
    cmp(&[".", "-T", "--write-thread"], *FASTA, &FASTA);
}
