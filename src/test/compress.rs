use super::*;

#[test]
fn compress_pipe() {
    Tester::new()
        .pipe(
            &[".", "--to", "fasta.gz", "--compr-level", "9"],
            &FASTA,
            &[".", "--fmt", "fasta.gz"],
            &FASTA,
        )
        .pipe(
            &[".", "--to", "fasta.bz2", "--compr-level", "9"],
            &FASTA,
            &[".", "--fmt", "fasta.bz2"],
            &FASTA,
        )
        .pipe(
            &[".", "--to", "fasta.lz4", "--compr-level", "9"],
            &FASTA,
            &[".", "--fmt", "fasta.lz4"],
            &FASTA,
        )
        .pipe(
            &[".", "--to", "fasta.zst", "--compr-level", "9"],
            &FASTA,
            &[".", "--fmt", "fasta.zst"],
            &FASTA,
        );
}

#[test]
fn compress_file() {
    let t = Tester::new();

    t.temp_file("compr.fa.gz", None, |path, _| {
        t.succeeds(&[".", "-o", path], *FASTA);
        t.fails(&[".", "--fmt", "fasta"], path, "FASTA parse error");
        t.cmp(&["."], FileInput(path), *FASTA);
        t.cmp(&[".", "--fmt", "fasta.gz"], FileInput(path), *FASTA);
    });
}
