
use super::*;


#[test]
fn head() {
    Tester::new()
        .cmp(&["head", "-n", "3"], *FASTA, &SEQS[..3].concat());
}
