
use std::fs::File;

use super::*;


#[test]
fn concat() {
    let t = Tester::new();

    t.temp_dir("concat", |tmp_dir| {
        let p = tmp_dir.path();

        let p1 = p.join("f1.fq");;
        let mut f1 = File::create(&p1).unwrap();
        f1.write(b"@id1\nAAA\n+\nAAA\n@id2\nAAA\n+\nAAA\n").unwrap();
        f1.flush().unwrap();

        let p2 = p.join("f2.fq");;
        let mut f2 = File::create(&p2).unwrap();
        f2.write(b"@id1\nBBB\n+\nBBB\n@id2\nBBB\n+\nBBB\n").unwrap();
        f2.flush().unwrap();

        let p3 = p.join("f3.fq");;
        let mut f3 = File::create(&p3).unwrap();
        f3.write(b"@id1\nCCC\n+\nCCC\n@id2\nCCC\n+\nCCC\n").unwrap();
        f3.flush().unwrap();

        let p4 = p.join("f4.fq");;
        let mut f4 = File::create(&p4).unwrap();
        f4.write(b"@id\n\n+\n\n@id\n\n+\n\n").unwrap();
        f4.flush().unwrap();

        let input = MultiFileInput(vec![
            p1.to_str().unwrap().to_string(),
            p2.to_str().unwrap().to_string(),
            p3.to_str().unwrap().to_string(),
        ]);

        t.cmp(&["concat"], input.clone(),
            "@id1\nAAABBBCCC\n+\nAAABBBCCC\n@id2\nAAABBBCCC\n+\nAAABBBCCC\n"
        );
        t.cmp(&["concat", "-s2"], input.clone(),
            "@id1\nAAANNBBBNNCCC\n+\nAAAJJBBBJJCCC\n@id2\nAAANNBBBNNCCC\n+\nAAAJJBBBJJCCC\n"
        );
        t.cmp(&["concat", "-s2", "-c", "-", "--q-char", "~"], input.clone(),
            "@id1\nAAA--BBB--CCC\n+\nAAA~~BBB~~CCC\n@id2\nAAA--BBB--CCC\n+\nAAA~~BBB~~CCC\n"
        );

        t.fails(
            &["concat"],
            MultiFileInput(vec![p1.to_str().unwrap().to_string(), p4.to_str().unwrap().to_string()]),
            "ID of record #2 (id) does not match the ID of the first one (id1)"
        );
    });
}
