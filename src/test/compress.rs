
use super::*;


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
