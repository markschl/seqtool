
use super::*;


#[test]
fn slice() {
    Tester::new()
        .cmp(&["slice", "-r", ".."], *FASTA, &FASTA)
        .cmp(&["slice", "-r", "1.."], *FASTA, &FASTA)
        .cmp(&["slice", "-r", "..2"], *FASTA, &SEQS[..2].concat())
        .cmp(&["slice", "-r", "1..2"], *FASTA, &SEQS[..2].concat())
        .cmp(&["slice", "-r", "2..3"], *FASTA, &SEQS[1..3].concat());
}
