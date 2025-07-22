use super::*;

#[test]
fn head() {
    cmp(&["head", "-n", "3"], &*FASTA, &SEQS[..3].concat());
}
