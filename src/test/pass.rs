
use super::*;


#[test]
fn pass() {
    Tester::new()
        .cmp(&["pass"], *FASTA, &FASTA)
        .cmp(&["."], *FASTA, &FASTA);
}

#[test]
fn pass_fasta() {
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
fn pass_other() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    Tester::new()
        .cmp(&[".", "--fq"], fq, fq)
        .cmp(&[".", "--txt", "id,seq", "--to-txt", "id,seq"], txt, txt)
    // convert
        .cmp(&[".", "--to-txt", "id,seq"], fa, txt)
        .cmp(&[".", "--fq", "--to-fa"], fq, fa)
    //    .cmp(&[".", "--fq", "--to-txt", "id,seq,qual"], fq, txt_qual)
        .cmp(&[".", "--txt", "id,seq", "--to-fa"], txt, fa)
    //    .cmp(&[".", "--txt", "id,seq,qual", "--to-fq"], txt_qual, fq)
        .fails(&[".", "--to-fq"], fa, "Qualities missing")
        .fails(&[".", "--txt", "id,seq", "--to-fq"], txt, "Qualities missing");
}

#[test]
fn pipe() {
    Tester::new().pipe(&["."], &FASTA, &["."], &FASTA);
}

#[test]
fn compress() {
    Tester::new()
        .pipe(
            &[".", "--outformat", "fasta.gz", "--compr-level", "9"], &FASTA,
            &[".", "--format", "fasta.gz"], &FASTA
        )
        .pipe(
            &[".", "--outformat", "fasta.bz2", "--compr-level", "9"], &FASTA,
            &[".", "--format", "fasta.bz2"], &FASTA
        )
        .pipe(
            &[".", "--outformat", "fasta.lz4", "--compr-level", "9"], &FASTA,
            &[".", "--format", "fasta.lz4"], &FASTA
        )
        .pipe(
            &[".", "--outformat", "fasta.zst", "--compr-level", "9"], &FASTA,
            &[".", "--format", "fasta.zst"], &FASTA
        );
}

#[test]
fn attrs() {
    let fa = ">seq;a=0 b=3\nATGC\n";
    Tester::new()
        .cmp(&[".", "--to-txt", "a:p"], *FASTA, "2\n1\n10\n11\n")
        .cmp(&[".", "--to-txt", "a:b"], fa, "3\n")
        .cmp(&[".", "--to-txt", "a:a"], fa, "\"\"")
        .cmp(&[".", "--to-txt", "a:a", "--adelim", ";"], fa, "0\n")
        .cmp(&[".", "--to-txt", "a:b", "--adelim", ";"], fa, "\"\"")
        .cmp(&[".", "--to-txt", "id,desc,seq"], fa, "seq;a=0\tb=3\tATGC\n")
        .cmp(&[".", "-a", "b={a:a}", "--adelim", ";"], fa, ">seq;a=0;b=0 b=3\nATGC\n")
        .cmp(&[".", "-a", "c={a:b}"], fa, ">seq;a=0 b=3 c=3\nATGC\n")
        .cmp(&[".", "-a", "c={a:-b}"], fa, ">seq;a=0 c=3\nATGC\n");
}
