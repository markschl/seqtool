// according to IUPAC https://iubmb.qmul.ac.uk/misc/naseq.html#500, Table 1
// whereby ambiguity codes completely contained in another are also included
// (e.g. V matches M, R and S i naddition to A, C and G)
pub static AMBIG_DNA: &[(u8, &[u8])] = &[
    (b'M', b"AC"),
    (b'R', b"AG"),
    (b'W', b"AT"),
    (b'S', b"CG"),
    (b'Y', b"CT"),
    (b'K', b"GT"),
    (b'V', b"ACGMRS"),
    (b'H', b"ACTMWY"),
    (b'D', b"AGTRWK"),
    (b'B', b"CGTSYK"),
    (b'N', b"ACGTMRWSYKVHDB"),
];

// same as DNA, T -> U
pub static AMBIG_RNA: &[(u8, &[u8])] = &[
    (b'M', b"AC"),
    (b'R', b"AG"),
    (b'W', b"AU"),
    (b'S', b"CG"),
    (b'Y', b"CU"),
    (b'K', b"GU"),
    (b'V', b"ACGMRS"),
    (b'H', b"ACUMWY"),
    (b'D', b"AGURWK"),
    (b'B', b"CGUSYK"),
    (b'N', b"ACGUMRWSYKVHDB"),
];

// according to IUPAC, https://iupac.qmul.ac.uk/AminoAcid/A2021.html#AA212
pub static AMBIG_PROTEIN: &[(u8, &[u8])] = &[
    (b'B', b"DN"),
    // note: B and Z are matched by X as well
    (b'X', b"ARNDCEQGHILKMFPSTWYVBZ"),
    (b'Z', b"EQ"),
];
