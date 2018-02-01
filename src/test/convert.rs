
use super::*;


#[test]
fn convert() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    Tester::new()
        .cmp(&[".", "--fq"], fq, fq)
        .cmp(&[".", "--txt", "id,seq", "--to-txt", "id,seq"], txt, txt)
        .cmp(&[".", "--to-txt", "id,seq"], fa, txt)
        .cmp(&[".", "--fq", "--to-fa"], fq, fa)
        .cmp(&[".", "--txt", "id,seq", "--to-fa"], txt, fa)
        .fails(&[".", "--to-fq"], fa, "Qualities missing")
        .fails(&[".", "--txt", "id,seq", "--to-fq"], txt, "Qualities missing");
}

#[test]
fn txt_input() {
    let txt = "seq1\tATGC\tdesc1\nseq2\tATGC\tdesc2\n";
    let csv = txt.replace('\t', ",");
    let txt_header = format!("i\ts\td\n{}", txt);

    Tester::new()
        .cmp(&[".", "--txt", "id,seq,desc", "--to-txt", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--format", "txt", "--fields", "id,seq,desc", "--outformat", "txt", "--outfields", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--csv", "id,seq,desc", "--to-txt", "id,seq,desc"], &csv, txt)
        .cmp(&[".", "--csv", "id,seq,desc", "--to-csv", "id,seq,desc"], &csv, &csv)
        .cmp(&[".", "--txt", "id:1,desc:3,seq:2", "--to-txt", "id,seq,desc"], txt, txt)
        .cmp(&[".", "--txt", "id:i,desc:d,seq:s", "--to-txt", "id,seq,desc"], &txt_header, txt);
}
