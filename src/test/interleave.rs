use super::*;

#[test]
fn interleave() {
    let t = Tester::new();

    t.temp_file("file.fa", Some(*FASTA), |path, _| {
        t.cmp(
            &["interleave"],
            MultiFileInput(vec![path.to_string(), path.to_string()]),
            records!(0, 0, 1, 1, 2, 2, 3, 3),
        );
    });
}
