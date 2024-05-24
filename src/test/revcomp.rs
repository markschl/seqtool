use super::*;

#[test]
fn revcomp() {
    Tester::new()
        // DNA with ambiguities
        .cmp(
            &["revcomp"],
            ">id\nAGCT\nYRWS\nKMDV\nHBN\n",
            ">id\nNVDBHKMSWYRAGCT\n",
        )
        // RNA
        .cmp(
            &["revcomp"],
            ">id\nAGCU\nYRWS\nKMDV\nHBN\n",
            ">id\nNVDBHKMSWYRAGCU\n",
        )
        // mixed / protein
        .fails(
            &["revcomp"],
            ">id\nTX\n",
            "Only DNA/RNA sequences can be reverse-complemented",
        )
        // with explicitly set sequence type, invalid letters are left untouched
        .cmp(&["revcomp", "--seqtype", "dna"], ">id\nUA\n", ">id\nTU\n");
}

#[test]
fn revcomp_qual() {
    let fq = "@seq\nANCT\n+\n1234\n";
    let rc = "@seq\nAGNT\n+\n4321\n";
    Tester::new().cmp(&["revcomp", "--fq"], fq, rc);
}
