
use super::*;


#[test]
fn revcomp() {
    let fa = ">seq\nAGCT\nYRWS\nKMDV\nHBN\n";
    Tester::new()
        .cmp(&["revcomp"], fa, ">seq\nNVDBHKMSWYRAGCT\n");
}

#[test]
fn revcomp_qual() {
    let fq = "@seq\nANCT\n+\n1234\n";
    let rc = "@seq\nAGNT\n+\n4321\n";
    Tester::new()
        .cmp(&["revcomp", "--fq"], fq, rc);
}
