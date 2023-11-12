use super::*;

#[test]
fn set() {
    let fasta = ">seq\nATGC\n";
    Tester::new()
        .cmp(&["set", "-i", "seq2"], fasta, ">seq2\nATGC\n")
        .cmp(&["set", "-d", "desc"], fasta, ">seq desc\nATGC\n")
        .cmp(&["set", "-s", "NNNN"], fasta, ">seq\nNNNN\n");
}
