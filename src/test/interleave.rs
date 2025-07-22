use super::*;

#[test]
fn interleave() {
    with_tmpdir("st_interleave_", |td| {
        cmp(
            &["interleave"],
            td.multi_file(".fasta", vec![&&*FASTA, &&*FASTA]),
            records!(0, 0, 1, 1, 2, 2, 3, 3),
        );
    });
}
