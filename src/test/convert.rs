use std::fs::File;

use super::*;

#[test]
fn convert() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    Tester::new()
        .cmp(&[".", "--fq"], fq, fq)
        .cmp(&[".", "--tsv", "id,seq", "--to-tsv", "id,seq"], txt, txt)
        .cmp(&[".", "--tsv", "id,seq"], txt, txt)
        .cmp(&[".", "--tsv", "id,seq", "--to", "tsv"], txt, txt)
        .cmp(&[".", "--tsv", "id,seq", "--to", "csv"], txt, "seq,ATGC\n")
        // no input fields -> fall back to id,desc,seq
        .cmp(&[".", "--to", "tsv"], fa, "seq\t\tATGC\n")
        .cmp(&[".", "--to-tsv", "id,seq"], fa, txt)
        .cmp(&[".", "--fq", "--to-fa"], fq, fa)
        .cmp(&[".", "--tsv", "id,seq", "--to-fa"], txt, fa)
        .fails(&[".", "--to-fq"], fa, "No quality scores")
        .fails(
            &[".", "--tsv", "id,seq", "--to-fq"],
            txt,
            "No quality scores",
        );
}

// TODO: ST_FORMAT reomved
// #[test]
// fn var_format() {
//     let fa = ">seq\nATGC\n";
//     let fq = "@seq\nATGC\n+\nXXXX\n";
//     let tsv = "seq\tATGC\n";

//     let mut t = Tester::new();

//     t.var("ST_FORMAT", "fasta").cmp(&["."], fa, fa);
//     t.var("ST_FORMAT", "fastq").cmp(&["."], fq, fq);
//     t.var("ST_FORMAT", "tsv:id,seq").cmp(&["."], tsv, tsv);
//     t.var("ST_FORMAT", "fastq").cmp(&[".", "--to-fa"], fq, fa);
//     t.var("ST_FORMAT", "fastq")
//         .cmp(&[".", "--to-tsv", "id,seq"], fq, tsv);
// }

#[test]
fn txt_input() {
    let txt = "seq1\tATGC\tdesc1\nseq2\tATGC\tdesc2\n";
    let csv = txt.replace('\t', ",");
    let txt_header = format!("i\ts\td\n{}", txt);

    Tester::new()
        .cmp(
            &[".", "--tsv", "id,seq,desc", "--to-tsv", "id,seq,desc"],
            txt,
            txt,
        )
        .cmp(
            &[
                ".",
                "--fmt",
                "tsv",
                "--fields",
                "id,seq,desc",
                "--to",
                "tsv",
                "--outfields",
                "id,seq,desc",
            ],
            txt,
            txt,
        )
        .cmp(
            &[".", "--csv", "id,seq,desc", "--to-tsv", "id,seq,desc"],
            &csv,
            txt,
        )
        .cmp(
            &[".", "--csv", "id,seq,desc", "--to-csv", "id,seq,desc"],
            &csv,
            &csv,
        )
        .cmp(
            &[".", "--tsv", "id:1,desc:3,seq:2", "--to-tsv", "id,seq,desc"],
            txt,
            txt,
        )
        .cmp(
            &[".", "--tsv", "id:i,desc:d,seq:s", "--to-tsv", "id,seq,desc"],
            &txt_header,
            txt,
        );
}

#[test]
fn qual_convert() {
    Tester::new()
        // Sanger -> Illumina 1.3+
        // qual. in second record are truncated automatically
        .cmp(
            &[".", "--fq", "--to", "fq-illumina"],
            &fq_records([33, 53, 73], [93, 103, 126]),
            &fq_records([64, 84, 104], [124, 126, 126]),
        )
        // Illumina 1.3+ -> Sanger
        .cmp(
            &[".", "--fq-illumina", "--to", "fq"],
            &fq_records([64, 84, 104], [124, 126]),
            &fq_records([33, 53, 73], [93, 95]),
        )
        // Sanger -> Solexa
        .cmp(
            &[".", "--fq", "--to", "fq-solexa"],
            &fq_records([33, 34, 42, 43, 73], [93, 103, 126]),
            &fq_records([59, 59, 72, 74, 104], [124, 126, 126]),
        )
        // Solexa -> Sanger
        .cmp(
            &[".", "--fmt", "fq-solexa", "--to", "fq"],
            &fq_records([59, 72, 74, 104], [124, 126]),
            &fq_records([34, 42, 43, 73], [93, 95]),
        )
        // Illumina -> Solexa
        .cmp(
            &[".", "--fq-illumina", "--to", "fq-solexa"],
            &fq_records([64, 65, 73, 74, 104], [124, 126]),
            &fq_records([59, 59, 72, 74, 104], [124, 126]),
        )
        // Solexa -> Illumina
        .cmp(
            &[".", "--fmt", "fq-solexa", "--to", "fq-illumina"],
            &fq_records([59, 72, 74, 104], [124, 126]),
            &fq_records([65, 73, 74, 104], [124, 126]),
        )
        // Validation errors
        .fails(
            &[".", "--fq", "--to", "fq-illumina"],
            &fq_records([31], []),
            "Invalid quality",
        )
        .fails(
            &[".", "--fq", "--to", "fq-illumina"],
            &fq_records([127], []),
            "Invalid quality",
        )
        .fails(
            &[".", "--fq-illumina", "--to", "fq"],
            &fq_records([63], []),
            "Invalid quality",
        )
        .fails(
            &[".", "--fq-illumina", "--to", "fq"],
            &fq_records([127], []),
            "Invalid quality",
        )
        .fails(
            &[".", "--fmt", "fq-solexa", "--to", "fq"],
            &fq_records([58], []),
            "Invalid quality",
        )
        .fails(
            &[".", "--fmt", "fq-solexa", "--to", "fq"],
            &fq_records([127], []),
            "Invalid quality",
        );
}

#[test]
fn qfile() {
    let fa = ">seq\nATGC\n";
    let qual = ">seq\n40 40 40 30\n";

    let t = Tester::new();

    t.temp_file("qfile.qual", Some(qual), |p, _| {
        t.cmp(&[".", "--qual", p, "--to-fq"], fa, "@seq\nATGC\n+\nIII?\n");
    });

    t.temp_file("qfile.qual", Some(qual), |p, _| {
        t.temp_file("qfile_out.qual", None, |p2, _| {
            t.cmp(&[".", "--qual", p, "--qual-out", p2], fa, fa);
            let mut qout = "".to_string();
            File::open(p2).unwrap().read_to_string(&mut qout).unwrap();
            assert_eq!(qout, qual);
        });
    });

    t.temp_file("qfile.qual", Some(">seq1\n40 40 40 30\n"), |p, _| {
        t.fails(
            &[".", "--qual", p],
            ">seq1\nATGC\n>seq2\nATGC\n",
            "Quality scores in QUAL file missing for record 'seq2'",
        );
    });

    t.temp_file("qfile.qual", Some(">seq\n40\n"), |p, _| {
        t.fails(&[".", "--qual", p], fa, "is not equal to sequence length");
    });

    t.temp_file("qfile.qual", Some(">seq2\n40 40 40 30\n"), |p, _| {
        t.fails(&[".", "--qual", p], fa, "ID mismatch");
    });

    t.temp_file("qfile.qual", Some(">seq\n40 40 40  30\n"), |p, _| {
        t.fails(&[".", "--qual", p], fa, "Invalid quality score");
    });
}
