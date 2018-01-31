
use super::*;


#[test]
fn filter() {
    let fa = ">id\nSEQ\n>id2 a=20\nSEQ\n>id3 a=\nSEQ";
    Tester::new()
        .cmp(&["filter", "s:seqlen > s:ungapped_len and a:p >= 10"], *FASTA, &SEQS[2..].concat())
        .cmp(&["filter", ".id == 'seq0'"], *FASTA, SEQS[1])
        .cmp(&["filter", "not(def(id))"], *FASTA, "")
        .cmp(&["filter", "def(a:a) and a:a >= 20", "--to-txt", "id"], fa, "id2\n")
        .cmp(&["filter", "a:a >= 20", "--to-txt", "id"], fa, "id2\n")
        .cmp(&["filter", ".id like 'id*'", "--to-txt", "id"], fa, "id\nid2\nid3\n");
}
