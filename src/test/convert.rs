use super::*;

#[test]
fn convert() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    cmp(&[".", "--fq"], fq, fq);
    cmp(&[".", "--tsv", "id,seq", "--to-tsv", "id,seq"], txt, txt);
    cmp(&[".", "--tsv", "id,seq"], txt, txt);
    cmp(&[".", "--tsv", "id,seq", "--to", "tsv"], txt, txt);
    cmp(&[".", "--tsv", "id,seq", "--to", "csv"], txt, "seq,ATGC\n");
    // no input fields -> fall back to id,desc,seq
    cmp(&[".", "--to", "tsv"], fa, "seq\t\tATGC\n");
    cmp(&[".", "--to-tsv", "id,seq"], fa, txt);
    cmp(&[".", "--fq", "--to-fa"], fq, fa);
    cmp(&[".", "--tsv", "id,seq", "--to-fa"], txt, fa);
    fails(&[".", "--to-fq"], fa, "No quality scores");
    fails(
        &[".", "--tsv", "id,seq", "--to-fq"],
        txt,
        "No quality scores",
    );
}

#[test]
fn var_format() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let tsv = "seq\tATGC\n";

    cmp_with_env(&["."], fa, fa, [("ST_FORMAT", "fasta")]);
    cmp_with_env(&[".", "--fq"], fq, fq, [("ST_FORMAT", "fastq")]);
    // env("ST_FORMAT", "tsv:id,seq")cmp(&["."], tsv, tsv);
    cmp_with_env(&[".", "--fq", "--to-fa"], fq, fa, [("ST_FORMAT", "fastq")]);

    cmp_with_env(
        &[".", "--fq", "--to-tsv", "id,seq"],
        fq,
        tsv,
        [("ST_FORMAT", "fasta")],
    );
}

#[test]
fn txt_input() {
    let txt = "seq1\tATGC\tdesc1\nseq2\tATGC\tdesc2\n";
    let csv = txt.replace('\t', ",");
    let txt_header = format!("i\ts\td\n{txt}");

    cmp(
        &[".", "--tsv", "id,seq,desc", "--to-tsv", "id,seq,desc"],
        txt,
        txt,
    );
    cmp(
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
    );
    cmp(
        &[".", "--csv", "id,seq,desc", "--to-tsv", "id,seq,desc"],
        &csv,
        txt,
    );
    cmp(
        &[".", "--csv", "id,seq,desc", "--to-csv", "id,seq,desc"],
        &csv,
        &csv,
    );
    cmp(&[".", "--csv", "id,seq,desc", "--to", "csv"], &csv, &csv);
    cmp(&[".", "--csv", "id,seq,desc", "--to", "tsv"], &csv, txt);
    cmp(
        &[".", "--tsv", "id:1,desc:3,seq:2", "--to-tsv", "id,seq,desc"],
        txt,
        txt,
    );
    cmp(
        &[".", "--tsv", "id:i,desc:d,seq:s", "--to-tsv", "id,seq,desc"],
        &txt_header,
        txt,
    );
}

#[test]
fn qual_convert() {
    // Sanger -> Illumina 1.3+
    // qual. in second record are truncated automatically
    cmp(
        &[".", "--fq", "--to", "fq-illumina"],
        fq_records([33, 53, 73], [93, 103, 126]),
        &fq_records([64, 84, 104], [124, 126, 126]),
    );
    // Illumina 1.3+ -> Sanger
    cmp(
        &[".", "--fq-illumina", "--to", "fq"],
        fq_records([64, 84, 104], [124, 126]),
        &fq_records([33, 53, 73], [93, 95]),
    );
    // Sanger -> Solexa
    cmp(
        &[".", "--fq", "--to", "fq-solexa"],
        fq_records([33, 34, 42, 43, 73], [93, 103, 126]),
        &fq_records([59, 59, 72, 74, 104], [124, 126, 126]),
    );
    // Solexa -> Sanger
    cmp(
        &[".", "--fmt", "fq-solexa", "--to", "fq"],
        fq_records([59, 72, 74, 104], [124, 126]),
        &fq_records([34, 42, 43, 73], [93, 95]),
    );
    // Illumina -> Solexa
    cmp(
        &[".", "--fq-illumina", "--to", "fq-solexa"],
        fq_records([64, 65, 73, 74, 104], [124, 126]),
        &fq_records([59, 59, 72, 74, 104], [124, 126]),
    );
    // Solexa -> Illumina
    cmp(
        &[".", "--fmt", "fq-solexa", "--to", "fq-illumina"],
        fq_records([59, 72, 74, 104], [124, 126]),
        &fq_records([65, 73, 74, 104], [124, 126]),
    );
    // Validation errors
    fails(
        &[".", "--fq", "--to", "fq-illumina"],
        fq_records([31], []),
        "Invalid quality",
    );
    fails(
        &[".", "--fq", "--to", "fq-illumina"],
        fq_records([127], []),
        "Invalid quality",
    );
    fails(
        &[".", "--fq-illumina", "--to", "fq"],
        fq_records([63], []),
        "Invalid quality",
    );
    fails(
        &[".", "--fq-illumina", "--to", "fq"],
        fq_records([127], []),
        "Invalid quality",
    );
    fails(
        &[".", "--fmt", "fq-solexa", "--to", "fq"],
        fq_records([58], []),
        "Invalid quality",
    );
    fails(
        &[".", "--fmt", "fq-solexa", "--to", "fq"],
        fq_records([127], []),
        "Invalid quality",
    );
}

#[test]
fn qfile() {
    with_tmpdir("st_qfile_", |td| {
        let fa = ">seq\nATGC\n";
        let qual = ">seq\n40 40 40 30\n";

        let qfile = td.file(".qual", qual);
        cmp(
            &[".", "--qual", &qfile, "--to-fq"],
            fa,
            "@seq\nATGC\n+\nIII?\n",
        );

        let qfile_out = td.path("qfile_out.qual");
        cmp(&[".", "--qual", &qfile, "--qual-out", &qfile_out], fa, fa);
        assert_eq!(&qfile_out.content(), qual);

        let qfile = td.file(".qual", ">seq1\n40 40 40 30\n");
        fails(
            &[".", "--qual", &qfile],
            ">seq1\nATGC\n>seq2\nATGC\n",
            "Quality scores in QUAL file missing for record 'seq2'",
        );

        let qfile = td.file(".qual", ">seq\n40\n");
        fails(
            &[".", "--qual", &qfile],
            fa,
            "is not equal to sequence length",
        );

        let qfile = td.file(".qual", ">seq2\n40 40 40 30\n");
        fails(&[".", "--qual", &qfile], fa, "ID mismatch");

        let qfile = td.file(".qual", ">seq\n40 40 40  30\n");
        fails(&[".", "--qual", &qfile], fa, "Invalid quality score");
    });
}
