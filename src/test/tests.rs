
use std::str;
use std::fs::File;
use std::io::Read;
use itertools::Itertools;
use seq_io::fasta::{self, Record};
use assert_cli::Assert;

use super::*;

/// Tests

// To debug/reproduce: `echo "<input>" | tr '\\n' '\n' | cargo run ... | cat -e`

#[test]
fn pass() {
    cmp_stdout!(&["pass"], FASTA, FASTA);
    cmp_stdout!(&["."], FASTA, FASTA);
}

#[test]
fn pass_fasta() {
    let fa = ">seq\nATGC\n";
    let fa_wrap = ">seq\nAT\nGC\n";
    let fa_wrap3 = ">seq\nATG\nC\n";
    cmp_stdout!(&["."], fa, fa);
    cmp_stdout!(&["."], fa_wrap, fa);
    cmp_stdout!(&[".", "--wrap", "2"], fa, fa_wrap);
    cmp_stdout!(&[".", "--wrap", "3"], fa_wrap, fa_wrap3);
}

#[test]
fn pass_other() {
    let fa = ">seq\nATGC\n";
    let fq = "@seq\nATGC\n+\nXXXX\n";
    let txt = "seq\tATGC\n";

    cmp_stdout!(&[".", "--fq"], fq, fq);
    cmp_stdout!(&[".", "--txt", "id,seq", "--to-txt", "id,seq"], txt, txt);
    // convert
    cmp_stdout!(&[".", "--to-txt", "id,seq"], fa, txt);
    cmp_stdout!(&[".", "--fq", "--to-fa"], fq, fa);
    //cmp_stdout!(&[".", "--fq", "--to-txt", "id,seq,qual"], fq, txt_qual);
    cmp_stdout!(&[".", "--txt", "id,seq", "--to-fa"], txt, fa);
    //cmp_stdout!(&[".", "--txt", "id,seq,qual", "--to-fq"], txt_qual, fq);
    fails!(&[".", "--to-fq"], fa, "Qualities missing");
    fails!(
        &[".", "--txt", "id,seq", "--to-fq"],
        txt,
        "Qualities missing"
    );
}

#[test]
fn attrs() {
    cmp_stdout!(&[".", "--to-txt", "a:p"], FASTA, "2\n1\n10\n11\n");
    let fa = ">seq;a=0 b=3\nATGC\n";
    cmp_stdout!(&[".", "--to-txt", "a:b"], fa, "3\n");
    cmp_stdout!(&[".", "--to-txt", "a:a", "--adelim", ";"], fa, "0\n");
    cmp_stdout!(&[".", "-a", "b={a:a}", "--adelim", ";"], fa, ">seq;a=0;b=0 b=3\nATGC\n");
    cmp_stdout!(&[".", "-a", "c={a:b}"], fa, ">seq;a=0 b=3 c=3\nATGC\n");
    cmp_stdout!(&[".", "-a", "c={a:-b}"], fa, ">seq;a=0 c=3\nATGC\n");
}

#[test]
fn stats() {
    let seq = ">seq\nATGC-NYA\n";
    let retval = "seq\t8\t7\t40\t2\t3";
    let vars = "s:seqlen,s:ungapped_len,s:gc,s:count:A,s:count:AT";
    let vars_noprefix = vars.replace("s:", "");
    let retval2 = format!("id\t{}\n{}", vars_noprefix.replace(",", "\t"), retval);
    cmp_stdout!(&[".", "--to-txt", &format!("id,{}", vars)], seq, retval);
    cmp_stdout!(&["stat", &vars_noprefix], seq, retval2);
}

#[test]
fn count() {
    cmp_stdout!(&["count"], FASTA, "4\n");
    cmp_stdout!(&["count", "-k", "a:p"], FASTA, "1\t1\n10\t1\n11\t1\n2\t1\n");
    cmp_stdout!(
        &["count", "-k", "n:10:{a:p}"],
        FASTA, "(0,10]\t2\n(10,20]\t2\n"
    );
    cmp_stdout!(
        &["count", "-nk", "n:10:{a:p}"],
        FASTA, "0\t2\n10\t2\n"
    );
}

#[test]
fn slice() {
    cmp_stdout!(&["slice", "-r", ".."], FASTA, FASTA);
    cmp_stdout!(&["slice", "-r", "1.."], FASTA, FASTA);
    cmp_stdout!(&["slice", "-r", "..2"], FASTA, SEQS[..2].concat());
    cmp_stdout!(&["slice", "-r", "1..2"], FASTA, SEQS[..2].concat());
    cmp_stdout!(&["slice", "-r", "2..3"], FASTA, SEQS[1..3].concat());
}

#[test]
fn head() {
    cmp_stdout!(&["head", "-n", "3"], FASTA, SEQS[..3].concat());
}

#[test]
fn tail() {
    fails!(&["tail", "-n", "3"], FASTA, "Cannot use STDIN as input");
}

#[test]
fn upper() {
    let fa = ">seq\naTgC\n";
    cmp_stdout!(&["upper"], fa, ">seq\nATGC\n");
}

#[test]
fn lower() {
    let fa = ">seq\naTgC\n";
    cmp_stdout!(&["lower"], fa, ">seq\natgc\n");
}

#[test]
fn mask() {
    let fa = ">seq\nATGCa\ntgc\n";
    cmp_stdout!(&["mask", ".."], fa, ">seq\natgcatgc\n");
    cmp_stdout!(&["mask", "..2,-2.."], fa, ">seq\natGCatgc\n");
    cmp_stdout!(&["mask", "4.."], fa, ">seq\nATGcatgc\n");
    cmp_stdout!(&["mask", "--hard", "N", "4.."], fa, ">seq\nATGNNNNN\n");
    cmp_stdout!(
        &["mask", "--unmask", "4.."],
        ">seq\nATGcatgc\n",
        ">seq\nATGCATGC\n"
    );
}

#[test]
fn revcomp() {
    let fa = ">seq\nAGCT\nYRWS\nKMDV\nHBN\n";
    cmp_stdout!(&["revcomp"], fa, ">seq\nNVDBHKMSWYRAGCT\n");
}

#[test]
fn revcomp_qual() {
    let fq = "@seq\nANCT\n+\n1234\n";
    let rc = "@seq\nAGNT\n+\n4321\n";
    cmp_stdout!(&["revcomp", "--fq"], fq, rc);
}

#[test]
fn trim() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    cmp_stdout!(&["trim", ".."], fasta, fasta);
    cmp_stdout!(&["trim", "1.."], fasta, fasta);
    cmp_stdout!(&["trim", "..1"], fasta, fasta_record(&seq[..1]));
    cmp_stdout!(&["trim", "2..-2"], fasta, fasta_record(&seq[1..3]));

    cmp_stdout!(&["trim", "-e", "1..3"], fasta, fasta_record(&seq[1..2]));
    // empty seq
    cmp_stdout!(&["trim", "2..1"], fasta, fasta_record(""));
}

#[test]
fn trim0() {
    let seq = "ATGC";
    let fasta = fasta_record(seq);
    cmp_stdout!(&["trim", "-0", "1..3"], fasta, fasta_record(&seq[1..3]));
    cmp_stdout!(&["trim", "-0", "..3"], fasta, fasta_record(&seq[..3]));
    cmp_stdout!(&["trim", "-0", "2.."], fasta, fasta_record(&seq[2..]));
}

#[test]
fn trim_qual() {
    // quality trimming
    let fq = "@id\nATGC\n+\n1234\n";
    cmp_stdout!(&["trim", "--fq", "..2"], fq, "@id\nAT\n+\n12\n");
    cmp_stdout!(&["trim", "--fq", "2..3"], fq, "@id\nTG\n+\n23\n");
}

#[test]
fn trim_vars() {
    let id = "id start=2 end=3 range=2..3";
    let fa = format!(">{}\nATGC\n", id);
    let trimmed = format!(">{}\nTG\n", id);
    cmp_stdout!(&["trim", "{a:start}..{a:end}"], fa, &trimmed);
    cmp_stdout!(&["trim", "{a:range}"], fa, &trimmed);
}

#[test]
fn trim_multiline() {
    let fa = ">id\nAB\nCDE\nFGHI\nJ";
    let seq = "ABCDEFGHIJ";
    cmp_stdout!(&["trim", ".."], fa, format!(">id\n{}\n", seq));
    for start in 0..seq.len() - 1 {
        for end in start..seq.len() {
            cmp_stdout!(&["trim", "-0", &format!("{}..{}", start, end)], fa,
                        format!(">id\n{}\n", &seq[start..end]));
        }
    }
}

#[test]
fn set() {
    let fasta = ">seq\nATGC\n";
    cmp_stdout!(&["set", "-i", "seq2"], fasta, ">seq2\nATGC\n");
    cmp_stdout!(&["set", "-d", "desc"], fasta, ">seq desc\nATGC\n");
    cmp_stdout!(&["set", "-s", "NNNN"], fasta, ">seq\nNNNN\n");
}

#[test]
fn del() {
    let fasta = ">seq;p=0 a=1 b=2\nATGC\n";
    cmp_stdout!(&["del", "-d"], fasta, ">seq;p=0\nATGC\n");
    cmp_stdout!(&["del", "--attrs", "a,b"], fasta, ">seq;p=0\nATGC\n");
    cmp_stdout!(&["del", "--adelim", ";", "--attrs", "p"], fasta, ">seq a=1 b=2\nATGC\n");
}

#[test]
fn replace() {
    let fa = ">id_123 some desc\nATGC\n";
    cmp_stdout!(&["replace", "T", "U"], fa, ">id_123 some desc\nAUGC\n");
    cmp_stdout!(
        &["replace", "-r", "[AT]", "?"],
        fa,
        ">id_123 some desc\n??GC\n"
    );
    cmp_stdout!(
        &["replace", "-ir", r"_\d{3}", ".."],
        fa,
        ">id.. some desc\nATGC\n"
    );
    cmp_stdout!(
        &["replace", "-ir", r"_(\d{3})", "..$1"],
        fa,
        ">id..123 some desc\nATGC\n"
    );
    cmp_stdout!(
        &["replace", "-d", "e", "a"],
        fa,
        ">id_123 soma dasc\nATGC\n"
    );
}

// split

#[test]
fn split_n() {
    for size in 1..5 {
        let tmp_dir = ::std::env::temp_dir(); //  tempdir::TempDir::new("split_test").expect("Could not create temporary directory");
        let key = tmp_dir.join("f_{split:chunk}.{default_ext}");

        run!(&["split", "-n", &format!("{}", size), "-pk", &key.to_string_lossy()], FASTA)
            .succeeds()
            .unwrap();

        for (i, seqs) in _SEQS.iter().chunks(size).into_iter().enumerate() {
            let p = tmp_dir.join(format!("f_{}.fasta", i + 1));
            let mut reader =
                fasta::Reader::from_path(&p).expect(&format!("file {:?} not found", p));
            for seq in seqs {
                let rec = reader.next().expect("Not enough records").unwrap();
                assert_eq!(
                    &format!(
                        ">{} {}\n{}\n",
                        rec.id().unwrap(),
                        rec.desc().unwrap().unwrap(),
                        str::from_utf8(rec.seq()).unwrap()
                    ),
                    seq
                );
            }
            assert!(reader.next().is_none(), "Too many records");
        }
    }
}

#[test]
fn split_key() {
    let tmp_dir =
        tempdir::TempDir::new("split_test").expect("Could not create temporary directory");
    let subdir = tmp_dir.path().join("subdir");
    let expected: &[&str] = &["seq1_2", "seq0_1", "seq3_10", "seq2_11"];

    let key = &subdir.join("{id}_{a:p}.fa");
    run!(&["split", "-pk", &key.to_string_lossy()], FASTA)
        .succeeds()
        .unwrap();

    for (i, k) in expected.iter().enumerate() {
        let p = subdir.join(format!("{}.fa", k));
        let mut reader = fasta::Reader::from_path(&p).expect(&format!("file {:?} not found", p));
        let rec = reader.next().unwrap().unwrap().to_owned_record();
        assert_eq!(
            &format!(">{} {}\n{}\n", rec.id().unwrap(), rec.desc().unwrap().unwrap(),
                str::from_utf8(rec.seq()).unwrap()
            ),
            &SEQS[i]
        );
        assert!(reader.next().is_none());
    }
}

#[test]
fn split_key_seqlen() {
    let tmp_dir =
        tempdir::TempDir::new("split_test").expect("Could not create temporary directory");

    run!(&["split", "-pk", &tmp_dir.path().join("{s:seqlen}.fa").to_string_lossy()], FASTA)
        .succeeds()
        .unwrap();

    let mut f = File::open(tmp_dir.path().join("25.fa")).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    assert_eq!(&s, &FASTA as &str);
}

#[cfg(feature = "exprtk")]
#[test]
fn filter() {
    macro_rules! cmp_stdout_expr {
        ($args:expr, $input:expr, $cmp:expr) => {
            Assert::command(&["cargo", "run", "--features", "exprtk", "--"])
                .with_args($args)
                .stdin($input.as_ref())
                .stdout()
                .is($cmp.as_ref())
                .unwrap();
         };
    }

    cmp_stdout_expr!(&["filter", "s:seqlen > s:ungapped_len and a:p >= 10"], FASTA, SEQS[2..].concat());
    cmp_stdout_expr!(&["filter", ".id == 'seq0'"], FASTA, SEQS[1]);
    cmp_stdout_expr!(&["filter", "not(def(id))"], FASTA, "");
    let fa = ">id\nSEQ\n>id2 a=20\nSEQ\n>id3 a=\nSEQ";
    cmp_stdout_expr!(&["filter", "def(a:a) and a:a >= 20", "--to-txt", "id"], fa, "id2\n");
    cmp_stdout_expr!(&["filter", "a:a >= 20", "--to-txt", "id"], fa, "id2\n");
    cmp_stdout_expr!(&["filter", ".id like 'id*'", "--to-txt", "id"], fa, "id\nid2\nid3\n");
}
