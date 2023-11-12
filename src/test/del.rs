use super::*;

#[test]
fn del() {
    let fasta = ">seq;p=0 a=1 b=2\nATGC\n";
    Tester::new()
        .cmp(&["del", "-d"], fasta, ">seq;p=0\nATGC\n")
        .cmp(&["del", "--attrs", "a,b"], fasta, ">seq;p=0\nATGC\n")
        .cmp(
            &["del", "--adelim", ";", "--attrs", "p"],
            fasta,
            ">seq a=1 b=2\nATGC\n",
        );
}
