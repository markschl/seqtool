use super::*;

#[test]
fn concat() {
    with_tmpdir("st_concat_", |td| {
        let input = td.multi_file(
            ".fastq",
            [
                "@id1\nAAA\n+\nAAA\n@id2\nAAA\n+\nAAA\n",
                "@id1\nBBB\n+\nBBB\n@id2\nBBB\n+\nBBB\n",
                "@id1\nCCC\n+\nCCC\n@id2\nCCC\n+\nCCC\n",
            ],
        );

        cmp(
            &["concat"],
            &input,
            "@id1\nAAABBBCCC\n+\nAAABBBCCC\n@id2\nAAABBBCCC\n+\nAAABBBCCC\n",
        );
        cmp(
            &["concat", "-s2"],
            &input,
            "@id1\nAAANNBBBNNCCC\n+\nAAAJJBBBJJCCC\n@id2\nAAANNBBBNNCCC\n+\nAAAJJBBBJJCCC\n",
        );
        cmp(
            &["concat", "-s2", "-c", "-", "--q-char", "~"],
            &input,
            "@id1\nAAA--BBB--CCC\n+\nAAA~~BBB~~CCC\n@id2\nAAA--BBB--CCC\n+\nAAA~~BBB~~CCC\n",
        );

        // id mismatch
        fails(
            &["concat"],
            td.multi_file(".fasta", [">id1\nATG", ">id\nATG"]),
            "ID of record #2 (id) does not match the ID of the first one (id1)",
        );

        // too few records in second input
        fails(
            &["concat"],
            td.multi_file(".fasta", [">id1\nATG\n>id2\nA", ">id1\nATG"]),
            "The number of records in input #2 does not match the number of records in input #1",
        );

        // too many records in second input
        fails(
            &["concat"],
            td.multi_file(".fasta", [">id1\nATG", ">id1\nATG\n>id2\nA"]),
            "The number of records in input #2 does not match the number of records in input #1",
        );
    });
}
