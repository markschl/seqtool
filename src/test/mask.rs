use super::*;

#[test]
fn mask() {
    let fa = ">seq\nATGCa\ntgc\n";
    Tester::new()
        .cmp(&["mask", ":"], fa, ">seq\natgcatgc\n")
        .cmp(&["mask", ":2,-2:"], fa, ">seq\natGCatgc\n")
        .cmp(&["mask", "4:"], fa, ">seq\nATGcatgc\n")
        .cmp(&["mask", "--hard", "N", "4:"], fa, ">seq\nATGNNNNN\n")
        .cmp(
            &["mask", "--unmask", "4:"],
            ">seq\nATGcatgc\n",
            ">seq\nATGCATGC\n",
        );
}
