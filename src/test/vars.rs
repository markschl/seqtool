use super::*;

#[test]
fn attrs() {
    let fa = ">seq;a=0 b=3\nATGC\n";
    Tester::new()
        .cmp(&[".", "--to-tsv", "attr(p)"], *FASTA, "2\n1\n10\n11\n")
        .cmp(&[".", "--to-tsv", "attr(b)"], fa, "3\n")
        .cmp(&[".", "--to-tsv", "attr(a)"], fa, "\"\"")
        .cmp(&[".", "--to-tsv", "attr(a)", "--adelim", ";"], fa, "0\n")
        .cmp(&[".", "--to-tsv", "attr(b)", "--adelim", ";"], fa, "\"\"")
        .cmp(&[".", "--to-tsv", "id,desc,seq"], fa, "seq;a=0\tb=3\tATGC\n")
        .cmp(&[".", "-a", "b={attr(a)}", "--adelim", ";"], fa, ">seq;a=0;b=0 b=3\nATGC\n")
        .cmp(&[".", "-a", "c={attr(b)}"], fa, ">seq;a=0 b=3 c=3\nATGC\n")
        .cmp(&[".", "-a", "c={attr_del(b)}"], fa, ">seq;a=0 c=3\nATGC\n");
}

#[test]
fn lists() {
    let t = Tester::new();
    let list = "
seq1\t2
seq0\t1
seq3\t10
seq2\t11";
    t.temp_file("lists", Some(list), |p, _| {
        let path = p.to_str().unwrap();
        t.cmp(
            &[".", "-l", path, "--to-tsv", "{{ attr(p) - list_col(2) }}"],
            *FASTA,
            "0\n0\n0\n0\n",
        );
    });
    // unordered
    let list = "
seq3\t10
seq1\t2
seq0\t1
seq2\t11";
    t.temp_file("lists", Some(list), |p, _| {
        let path = p.to_str().unwrap();
        t.cmp(
            &[".", "-ul", path, "--to-tsv", "{{ attr(p) - list_col(2) }}"],
            *FASTA,
            "0\n0\n0\n0\n",
        );
    });
    // unordered + missing
    let list = "
seq3\t10
seq1\t2
seq2\t11";
    t.temp_file("lists", Some(list), |p, _| {
        let path = p.to_str().unwrap();
        t.cmp(
            &[".", "-uml", path, "--to-tsv", "{{ attr(p) - list_col(2) }}"],
            *FASTA,
            "0\n\n0\n0\n",
        );
    });
    t.temp_file("lists", Some(list), |p, _| {
        let path = p.to_str().unwrap();
        t.cmp(
            &[".", "-uml", path, "--to-tsv", "{{ has_entry }}"],
            *FASTA,
            "true\nfalse\ntrue\ntrue\n",
        );
    });
}
