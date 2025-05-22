use crate::helpers::NA;

use super::*;

#[test]
fn trim() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    Tester::new()
        .cmp(&["trim", ":"], &fasta, &fasta)
        .cmp(&["trim", "1:"], &fasta, &fasta)
        .cmp(&["trim", ":1"], &fasta, &fasta_record(&seq[..1]))
        .cmp(&["trim", "2:-2"], &fasta, &fasta_record(&seq[1..3]))
        // exclusive
        .cmp(&["trim", "-e", "1:3"], &fasta, &fasta_record(&seq[1..2]))
        .cmp(&["trim", "-e", "2:3"], &fasta, &fasta_record(&seq[2..2]))
        .cmp(&["trim", "-e", "2:4"], &fasta, &fasta_record(&seq[2..3]))
        // exclusive + unbounded
        .cmp(&["trim", "-e", ":3"], &fasta, &fasta_record(&seq[..2]))
        .cmp(&["trim", "-e", "2:"], &fasta, &fasta_record(&seq[2..]))
        // empty seq
        .cmp(&["trim", "2:1"], &fasta, &fasta_record(""));
}

#[test]
fn trim0() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    Tester::new()
        .cmp(&["trim", "-0", "1:3"], &fasta, &fasta_record(&seq[1..3]))
        .cmp(&["trim", "-0", ":3"], &fasta, &fasta_record(&seq[..3]))
        .cmp(&["trim", "-0", "2:"], &fasta, &fasta_record(&seq[2..]));
}

#[test]
fn trim_qual() {
    // quality trimming
    let fq = "@id\nATGC\n+\n1234\n";
    Tester::new()
        .cmp(&["trim", "--fq", ":2"], fq, "@id\nAT\n+\n12\n")
        .cmp(&["trim", "--fq", "2:3"], fq, "@id\nTG\n+\n23\n");
}

#[test]
fn trim_vars() {
    let id = "id start=2 end=3 range=2:3";
    let fa = format!(">{id}\nATGC\n");
    let trimmed = format!(">{id}\nTG\n");
    Tester::new()
        .cmp(&["trim", "{attr(start)}:{attr(end)}"], &fa, &trimmed)
        .cmp(&["trim", "{attr(range)}"], &fa, &trimmed)
        // multiple ranges
        // TODO: space not deleted
        .cmp(
            &["trim", "{attr_del(r)}"],
            ">id r=1:2,4:4\nATGC\n",
            ">id \nATC\n",
        );
}

#[test]
fn trim_multiline() {
    let fa = ">id\nAB\nCDE\nFGHI\nJ";
    let seq = "ABCDEFGHIJ";
    let t = Tester::new();
    t.cmp(&["trim", ":"], &fa, &format!(">id\n{seq}\n"));

    for start in 0..seq.len() - 1 {
        for end in start..seq.len() {
            t.cmp(
                &["trim", "-0", &format!("{start}:{end}")],
                &fa,
                &format!(">id\n{}\n", &seq[start..end]),
            );
        }
    }
}

#[test]
fn trim_multiline_multirange() {
    let fa = ">id\nAB\nC\nDE\nFGHI\nJ";
    Tester::new()
        .cmp(&["trim", "2:4,6:7"], &fa, ">id\nBCDFG\n")
        .cmp(&["trim", "-4:-3,-1:"], &fa, ">id\nGHJ\n");
}

#[test]
fn trim_na() {
    Tester::new()
        .cmp(&["trim", &format!("{NA}:")], ">id\nABCDE\n", ">id\nABCDE\n")
        .cmp(
            &["trim", &format!("{NA}:{NA}")],
            ">id\nABCDE\n",
            ">id\nABCDE\n",
        )
        .cmp(
            &["trim", "{opt_attr(s)}:{attr(e)}"],
            &format!(">id s={NA} e=3\nABCDE\n"),
            &format!(">id s={NA} e=3\nABC\n"),
        )
        .fails(
            &["trim", "{attr(s)}:{attr(e)}"],
            &format!(">id s={NA} e=3\nABCDE\n"),
            "reserved for missing values",
        )
        .cmp(
            &["trim", "{opt_attr(s)}:{opt_attr(e)}"],
            ">id s=3\nABCDE\n",
            ">id s=3\nCDE\n",
        )
        .fails(
            &["trim", "{opt_attr(s)}:{opt_attr(e)}"],
            ">id s=something\nABCDE\n",
            "Could not convert 'something' to an integer number",
        );
}
