use super::*;
use itertools::Itertools;

use std::str;

#[test]
fn chunks() {
    with_tmpdir("st_split_chunks_", |td| {
        for size in 1..5 {
            let key = td.persistent_path("f_{chunk}.{default_ext}");
            succeeds(&["split", "-n", &format!("{size}"), "-po", &key], *FASTA);

            for (i, seqs) in SEQS.iter().chunks(size).into_iter().enumerate() {
                let f = td.path(&format!("f_{}.fasta", i + 1));
                assert_eq!(f.content(), seqs.into_iter().join(""));
            }
        }
    });
}

#[test]
fn key() {
    with_tmpdir("st_split_key_", |td| {
        let out_path = td.persistent_path("{id}_{attr(p)}.fasta");
        succeeds(&["split", "-po", &out_path], *FASTA);

        let expected = &["seq1_2", "seq0_1", "seq3_10", "seq2_11"];

        for (name, seq) in expected.iter().zip(SEQS) {
            let f = td.path(&format!("{}.fasta", name));
            assert_eq!(f.content(), seq);
        }
    });
}

#[test]
fn seqlen_count() {
    with_tmpdir("st_split_sl_", |td| {
        let key = td.persistent_path("{seqlen}.fasta");
        succeeds(&["split", "-o", &key], *FASTA);

        let out = td.path("25.fasta");
        cmp(
            &["split", "-po", &key, "-c", "-"],
            *FASTA,
            &format!("{}\t4\n", out.as_str()),
        );
        assert_eq!(out.content(), &FASTA as &str);
    });
}

#[cfg(feature = "gz")]
#[test]
fn compression() {
    with_tmpdir("st_split_compr_", |td| {
        let key = td.persistent_path("{id}_{attr(p)}.fasta.gz");
        succeeds(&["split", "-po", &key], *FASTA);

        let expected = &["seq1_2", "seq0_1", "seq3_10", "seq2_11"];

        for (name, seq) in expected.iter().zip(SEQS) {
            let f = td.path(&format!("{}.fasta.gz", name));
            assert_eq!(f.gz_content(), seq);
        }
    });
}
