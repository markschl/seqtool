
use super::*;


#[test]
fn filter() {
    let fa = ">id\nSEQ\n>id2 a=20\nSEQ\n>id3 a=\nSEQ";
    Tester::new()
        .cmp(&["filter", "s:seqlen > s:ungapped_len and a:p >= 10"], *FASTA, &SEQS[2..].concat())
        .cmp(&["filter", ".id == 'seq0'"], *FASTA, SEQS[1])
        .cmp(&["filter", "not(def(id))"], *FASTA, "")
        .cmp(&["filter", "def(a:a) and a:a >= 20", "--to-tsv", "id"], fa, "id2\n")
        .cmp(&["filter", "a:a >= 20", "--to-tsv", "id"], fa, "id2\n")
        .cmp(&["filter", ".id like 'id*'", "--to-tsv", "id"], fa, "id\nid2\nid3\n");
}

#[test]
fn drop_file() {
    let t = Tester::new();
    t.temp_dir("find_drop", |d| {
        let out = d.path().join("dropped.fa");
        let out_path = out.to_str().expect("invalid path");

        let fa = ">id1\nSEQ\n>id2\nOTHER";
        t.cmp(
            &["filter", ".seq != 'SEQ'", "-a", "i={num}", "--dropped", out_path],
            fa,
            ">id2 i=2\nOTHER\n"
        );

        let mut f = File::open(out_path).expect("File not there");
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();

        assert_eq!(&s, ">id1 i=1\nSEQ\n");
    })
}
