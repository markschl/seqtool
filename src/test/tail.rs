use super::*;

#[test]
fn tail() {
    let t = Tester::new();
    t.fails(&["tail", "-n", "3"], *FASTA, "Cannot use STDIN as input");
    t.temp_file("tail", Some(*FASTA), |path, _| {
        t.cmp(
            &["tail", "-n", "2"],
            FileInput(path),
            &select_fasta(&[2, 3]),
        );
    });
}
