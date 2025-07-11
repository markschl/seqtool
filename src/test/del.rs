use super::*;

#[test]
fn del() {
    let fasta = ">seq;p=0 a=1 b=2\nATGC\n";

    cmp(&["del", "-d"], fasta, ">seq;p=0\nATGC\n");
    // TODO: the extra space should be removed
    cmp(&["del", "--attrs", "a,b"], fasta, ">seq;p=0 \nATGC\n");
    cmp(
        &["del", "--attrs", "p", "--attr-fmt", ";key=value"],
        fasta,
        ">seq a=1 b=2\nATGC\n",
    );
}
