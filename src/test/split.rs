
use std::fs::File;
use std::str;
use seq_io::fasta::{self, Record};
use itertools::Itertools;
use super::*;


#[test]
fn split_n() {
    let t = Tester::new();

    for size in 1..5 {

        t.temp_dir("split_n", |tmp_dir| {

            let key = tmp_dir.path().join("f_{split:chunk}.{default_ext}");

            t.succeeds(&["split", "-n", &format!("{}", size), "-po", &key.to_str().unwrap()], *FASTA);

            for (i, seqs) in SEQS.iter().chunks(size).into_iter().enumerate() {
                let p = tmp_dir.path().join(format!("f_{}.fasta", i + 1));
                let mut reader = fasta::Reader::from_path(&p)
                    .expect(&format!("file {:?} not found", p));
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
        });
    }
}


#[test]
fn split_key() {
    let t = Tester::new();

    t.temp_dir("split_key", |tmp_dir| {
        let subdir = tmp_dir.path().join("subdir");
        let expected: &[&str] = &["seq1_2", "seq0_1", "seq3_10", "seq2_11"];

        let key = &subdir.join("{id}_{a:p}.fa");

        t.succeeds(&["split", "-po", &key.to_string_lossy()], *FASTA);

        for (i, k) in expected.iter().enumerate() {
            let p = subdir.join(format!("{}.fa", k));
            let mut reader = fasta::Reader::from_path(&p)
                .expect(&format!("file {:?} not found", p));
            let rec = reader.next().unwrap().unwrap().to_owned_record();
            assert_eq!(
                &format!(">{} {}\n{}\n", rec.id().unwrap(), rec.desc().unwrap().unwrap(),
                    str::from_utf8(rec.seq()).unwrap()
                ),
                &SEQS[i]
            );
            assert!(reader.next().is_none());
        }
    });
}


#[test]
fn split_key_seqlen() {
    let t = Tester::new();
    t.temp_dir("split_key_seqlen", |tmp_dir| {
        let p = tmp_dir.path().join("{s:seqlen}.fa");
        t.succeeds(&["split", "-po", p.to_str().unwrap()], *FASTA);

        let mut f = File::open(tmp_dir.path().join("25.fa")).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(&s, &FASTA as &str);
    });
}


#[test]
fn split_compression() {
    let t = Tester::new();

    t.temp_dir("split_compression", |tmp_dir| {
        let subdir = tmp_dir.path().join("subdir");

        let key = &subdir.join("{id}_{a:p}.fa.gz");

        t.succeeds(&["split", "-po", &key.to_str().unwrap()], *FASTA);

        let expected: &[&str] = &["seq1_2", "seq0_1", "seq3_10", "seq2_11"];

        let f = MultiFileInput(expected.iter()
            .map(|e| subdir.join(e.to_string() + ".fa.gz")
            .to_string_lossy().into())
            .collect());

        t.fails(&[".", "--fmt", "fasta"], f.clone(),
            "FASTA parse error: expected '>' but found '\\u{1f}' at file start"
        );

        t.cmp(&["."], f, *FASTA);

    });
}
