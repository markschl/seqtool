use super::*;

#[test]
fn compress_pipe() {
    #[cfg(feature = "gz")]
    cmp_pipe(
        &[".", "--to", "fasta.gz", "--compr-level", "9"],
        &*FASTA,
        &[".", "--fmt", "fasta.gz"],
        &*FASTA,
    );

    #[cfg(feature = "bz2")]
    cmp_pipe(
        &[".", "--to", "fasta.bz2", "--compr-level", "9"],
        &*FASTA,
        &[".", "--fmt", "fasta.bz2"],
        &*FASTA,
    );

    #[cfg(feature = "lz4")]
    cmp_pipe(
        &[".", "--to", "fasta.lz4", "--compr-level", "9"],
        &*FASTA,
        &[".", "--fmt", "fasta.lz4"],
        &*FASTA,
    );

    #[cfg(feature = "zstd")]
    cmp_pipe(
        &[".", "--to", "fasta.zst", "--compr-level", "9"],
        &*FASTA,
        &[".", "--fmt", "fasta.zst"],
        &*FASTA,
    );
}

#[test]
#[cfg(feature = "gz")]
fn compress_file() {
    with_tmpdir("st_compress_", |td| {
        let f = td.path("compr_out.fa.gz");
        succeeds(&[".", "-o", &f], &*FASTA);
        fails(&[".", "--fmt", "fasta"], &f, "FASTA parse error");
        cmp(&["."], &f, &*FASTA);
        cmp(&[".", "--fmt", "fasta.gz"], &f, &*FASTA);
    });
}
