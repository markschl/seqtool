use super::*;

const FA1: &str = "\
1,AAA
2,AAA
3,CCC
5,CCC
7,TTT
8,ATG
9,TGA
";

const FA2: &str = "\
1,AAA
3,CCC
4,CCC
5,CCC
6,TTT
8,GGG
10,GAT
";

const STATS: &str = "\
common:  3
unique1: 4
unique2: 4
";

const STATS_ID: &str = "\
common:  4
unique1: 3
unique2: 3
";

const CATEGORY1: &str = "\
1,AAA,common
2,AAA,unique1
3,CCC,common
5,CCC,common
7,TTT,unique1
8,ATG,unique1
9,TGA,unique1
";

const CATEGORY2: &str = "\
1,AAA,common
3,CCC,common
4,CCC,unique2
5,CCC,common
6,TTT,unique2
8,GGG,unique2
10,GAT,unique2
";

const CATEGORY_ID1: &str = "\
1,AAA,common
2,AAA,unique1
3,CCC,common
5,CCC,common
7,TTT,unique1
8,ATG,common
9,TGA,unique1
";

const CATEGORY_ID2: &str = "\
1,AAA,common
3,CCC,common
4,CCC,unique2
5,CCC,common
6,TTT,unique2
8,GGG,common
10,GAT,unique2
";

const COMMON: &str = "\
1,AAA
3,CCC
5,CCC
";

const COMMON_ID1: &str = "\
1,AAA
3,CCC
5,CCC
8,ATG
";

const COMMON_ID2: &str = "\
1,AAA
3,CCC
5,CCC
8,GGG
";

const UNIQUE1: &str = "\
2,AAA
7,TTT
8,ATG
9,TGA
";

const UNIQUE_ID1: &str = "\
2,AAA
7,TTT
9,TGA
";

const UNIQUE2: &str = "\
4,CCC
6,TTT
8,GGG
10,GAT
";

const UNIQUE_ID2: &str = "\
4,CCC
6,TTT
10,GAT
";

#[test]
fn cmp_() {
    with_tmpdir("st_cmp_", |td| {
        let common1 = td.path("cmp_common1.csv");
        let common2 = td.path("cmp_common2.csv");
        let uniq1 = td.path("cmp_unique1.csv");
        let uniq2 = td.path("cmp_unique2.csv");

        let input = td.multi_file(".csv", [FA1, FA2]);

        // compare by ID and sequence
        let cli = &[
            "cmp",
            "--csv",
            "id,seq",
            "--common1",
            &common1,
            "--common2",
            &common2,
            "--unique1",
            &uniq1,
            "--unique2",
            &uniq2,
        ];
        cmd(cli, &input).stderr(STATS);
        assert_eq!(common1.content(), COMMON);
        assert_eq!(common2.content(), COMMON);
        assert_eq!(uniq1.content(), UNIQUE1);
        assert_eq!(uniq2.content(), UNIQUE2);

        // compare by ID only
        let cli = &[
            "cmp",
            "-k",
            "id",
            "--csv",
            "id,seq",
            "--common1",
            &common1,
            "--common2",
            &common2,
            "--unique1",
            &uniq1,
            "--unique2",
            &uniq2,
        ];
        cmd(cli, &input).stderr(STATS_ID);
        assert_eq!(common1.content(), COMMON_ID1);
        assert_eq!(common2.content(), COMMON_ID2);
        assert_eq!(uniq1.content(), UNIQUE_ID1);
        assert_eq!(uniq2.content(), UNIQUE_ID2);
    });
}

#[test]
fn cmp_category() {
    with_tmpdir("st_cmp_category_", |td| {
        let cat1 = td.path("cmp_cat1.csv");
        let cat2 = td.path("cmp_cat2.csv");

        let input = td.multi_file(".csv", [FA1, FA2]);

        // compare by ID and sequence
        let cli = &[
            "cmp",
            "--csv",
            "id,seq",
            "--to-csv",
            "id,seq,category",
            "-o",
            &cat1,
            "--output2",
            &cat2,
        ];
        cmd(cli, &input).stderr(STATS);
        assert_eq!(cat1.content(), CATEGORY1);
        assert_eq!(cat2.content(), CATEGORY2);

        // compare by ID only
        let cli = &[
            "cmp",
            "-k",
            "id",
            "--csv",
            "id,seq",
            "--to-csv",
            "id,seq,category",
            "-o",
            &cat1,
            "--output2",
            &cat2,
        ];
        cmd(cli, &input).stderr(STATS_ID);
        assert_eq!(cat1.content(), CATEGORY_ID1);
        assert_eq!(cat2.content(), CATEGORY_ID2);
    });
}
