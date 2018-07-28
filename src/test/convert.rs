
use std::str;
use std::iter::repeat;
use std::convert::AsRef;

use super::*;


#[test]
fn convert() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    Tester::new()
        .cmp(&[".", "--fq"], fq, fq)
        .cmp(&[".", "--tsv", "id,seq", "--to-tsv", "id,seq"], txt, txt)
        .cmp(&[".", "--to-tsv", "id,seq"], fa, txt)
        .cmp(&[".", "--fq", "--to-fa"], fq, fa)
        .cmp(&[".", "--tsv", "id,seq", "--to-fa"], txt, fa)
        .fails(&[".", "--to-fq"], fa, "No quality scores")
        .fails(&[".", "--tsv", "id,seq", "--to-fq"], txt, "No quality scores");
}

#[test]
fn txt_input() {
    let txt = "seq1\tATGC\tdesc1\nseq2\tATGC\tdesc2\n";
    let csv = txt.replace('\t', ",");
    let txt_header = format!("i\ts\td\n{}", txt);

    Tester::new()
        .cmp(&[".", "--tsv", "id,seq,desc", "--to-tsv", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--fmt", "tsv", "--fields", "id,seq,desc", "--to", "tsv", "--outfields", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--csv", "id,seq,desc", "--to-tsv", "id,seq,desc"], &csv, txt)
        .cmp(&[".", "--csv", "id,seq,desc", "--to-csv", "id,seq,desc"], &csv, &csv)
        .cmp(&[".", "--tsv", "id:1,desc:3,seq:2", "--to-tsv", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--tsv", "id:i,desc:d,seq:s", "--to-tsv", "id,seq,desc"], &txt_header, txt);
}

#[test]
fn qual_convert() {
    fn make_records<Q1, Q2>(q1: Q1, q2: Q2) -> String
    where Q1: AsRef<[u8]>,
          Q2: AsRef<[u8]>
    {
        let q1 = q1.as_ref();
        let q2 = q2.as_ref();
        format!("@seq1\n{}\n+\n{}\n@seq2\n{}\n+\n{}\n",
            repeat('A').take(q1.len()).collect::<String>(),
            str::from_utf8(q1).unwrap(),
            repeat('G').take(q2.len()).collect::<String>(),
            str::from_utf8(q2).unwrap(),
        )
    }

    Tester::new()
        // Sanger -> Illumina 1.3
        // qual. in second record are truncated automatically
        .cmp(
            &[".", "--fq", "--to", "fq-illumina"],
            &make_records([33, 53,  73], [ 93, 103, 126]),
            &make_records([64, 84, 104], [124, 126, 126]),
        )
        // Illumina 1.3 -> Sanger
        .cmp(
            &[".", "--fq-illumina", "--to", "fq"],
            &make_records([64, 84, 104], [124, 126]),
            &make_records([33, 53,  73], [ 93,  95]),
        )
        // Sanger -> Solexa
        .cmp(
            &[".", "--fq", "--to", "fq-solexa"],
            &make_records([33, 34, 42, 43,  73], [ 93, 103, 126]),
            &make_records([59, 59, 72, 74, 104], [124, 126, 126]),
        )
        // Solexa -> Sanger
        .cmp(
            &[".", "--fmt", "fq-solexa", "--to", "fq"],
            &make_records([59, 72, 74, 104], [124, 126]),
            &make_records([34, 42, 43,  73], [ 93, 95]),
        )
        // Illumina -> Solexa
        .cmp(
            &[".", "--fq-illumina", "--to", "fq-solexa"],
            &make_records([64, 65, 73, 74, 104], [124, 126]),
            &make_records([59, 59, 72, 74, 104], [124, 126]),
        )
        // Solexa -> Illumina
        .cmp(
            &[".", "--fmt", "fq-solexa", "--to", "fq-illumina"],
            &make_records([59, 72, 74, 104], [124, 126]),
            &make_records([65, 73, 74, 104], [124, 126]),
        )
        // Validation errors
        .fails(&[".", "--fq", "--to", "fq-illumina"], &make_records([31], []), "Invalid quality")
        .fails(&[".", "--fq", "--to", "fq-illumina"], &make_records([127], []), "Invalid quality")
        .fails(&[".", "--fq-illumina", "--to", "fq"], &make_records([63], []), "Invalid quality")
        .fails(&[".", "--fq-illumina", "--to", "fq"], &make_records([127], []), "Invalid quality")
        .fails(&[".", "--fmt", "fq-solexa", "--to", "fq"], &make_records([58], []), "Invalid quality")
        .fails(&[".", "--fmt", "fq-solexa", "--to", "fq"], &make_records([127], []), "Invalid quality");
}
