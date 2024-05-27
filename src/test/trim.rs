use super::*;

#[test]
fn trim() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    Tester::new()
        .cmp(&["trim", ".."], &fasta, &fasta)
        .cmp(&["trim", "1.."], &fasta, &fasta)
        .cmp(&["trim", "..1"], &fasta, &fasta_record(&seq[..1]))
        .cmp(&["trim", "2..-2"], &fasta, &fasta_record(&seq[1..3]))
        .cmp(&["trim", "-e", "1..3"], &fasta, &fasta_record(&seq[1..2]))
        // empty seq
        .cmp(&["trim", "2..1"], &fasta, &fasta_record(""));
}

#[test]
fn trim0() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    Tester::new()
        .cmp(&["trim", "-0", "1..3"], &fasta, &fasta_record(&seq[1..3]))
        .cmp(&["trim", "-0", "..3"], &fasta, &fasta_record(&seq[..3]))
        .cmp(&["trim", "-0", "2.."], &fasta, &fasta_record(&seq[2..]));
}

#[test]
fn trim_qual() {
    // quality trimming
    let fq = "@id\nATGC\n+\n1234\n";
    Tester::new()
        .cmp(&["trim", "--fq", "..2"], fq, "@id\nAT\n+\n12\n")
        .cmp(&["trim", "--fq", "2..3"], fq, "@id\nTG\n+\n23\n");
}

#[test]
fn trim_vars() {
    let id = "id start=2 end=3 range=2..3";
    let fa = format!(">{}\nATGC\n", id);
    let trimmed = format!(">{}\nTG\n", id);
    Tester::new()
        .cmp(&["trim", "{attr(start)}..{attr(end)}"], &fa, &trimmed)
        .cmp(&["trim", "{attr(range)}"], &fa, &trimmed)
        // multiple ranges
        // TODO: space not deleted
        .cmp(&["trim", "{attr_del(r)}"], ">id r=1..2,4..4\nATGC\n", ">id \nATC\n");
}

#[test]
fn trim_multiline() {
    let fa = ">id\nAB\nCDE\nFGHI\nJ";
    let seq = "ABCDEFGHIJ";
    let t = Tester::new();
    t.cmp(&["trim", ".."], &fa, &format!(">id\n{}\n", seq));

    for start in 0..seq.len() - 1 {
        for end in start..seq.len() {
            t.cmp(
                &["trim", "-0", &format!("{}..{}", start, end)],
                &fa,
                &format!(">id\n{}\n", &seq[start..end]),
            );
        }
    }
}

#[test]
fn trim_multiline_multirange() {
    let fa = ">id\nAB\nC\nDE\nFGHI\nJ";
    let t = Tester::new();
    t.cmp(&["trim", "2..4,6..7"], &fa, ">id\nBCDFG\n");
    t.cmp(&["trim", "-4..-3,-1.."], &fa, ">id\nGHJ\n");
}
