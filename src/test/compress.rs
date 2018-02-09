
use super::*;


#[test]
fn compress_pipe() {
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
fn compress_file() {
    let t = Tester::new();

    t.temp_file("compr.fa.gz", None, |path, _| {
        t.succeeds(&[".", "-o", path], *FASTA);
        t.fails(&[".", "--format", "fasta"], path, "FASTA parse error");
        t.cmp(&["."], FileInput(path), *FASTA);
        t.cmp(&[".", "--format", "fasta.gz"], FileInput(path), *FASTA);
    });
}
