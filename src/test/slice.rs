use super::*;

#[test]
fn slice() {
    cmp(&["slice", ":"], &*FASTA, &*FASTA);
    cmp(&["slice", "1:"], &*FASTA, &*FASTA);
    cmp(&["slice", ":2"], &*FASTA, &SEQS[..2].concat());
    cmp(&["slice", "1:2"], &*FASTA, &SEQS[..2].concat());
    cmp(&["slice", "2:3"], &*FASTA, &SEQS[1..3].concat());
}
