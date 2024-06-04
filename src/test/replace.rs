use super::*;

static INPUT: &str = ">id_123 some desc\nA\nT\nGC\n";

#[test]
fn exact() {
    Tester::new()
        .cmp(&["replace", "T", "U"], INPUT, ">id_123 some desc\nAUGC\n")
        .cmp(
            &["replace", "T", "U"],
            ">a\nT\nT\n>b\nT\nT\n>c\nT\nT\n",
            ">a\nUU\n>b\nUU\n>c\nUU\n",
        )
        .cmp(
            &["replace", "ATG", "TGA"],
            INPUT,
            ">id_123 some desc\nTGAC\n",
        )
        .cmp(
            &["replace", "-d", "e", "a"],
            INPUT,
            ">id_123 soma dasc\nATGC\n",
        );
}

#[test]
fn regex() {
    Tester::new()
        .cmp(
            &["replace", "-r", "[AT]", "?"],
            INPUT,
            ">id_123 some desc\n??GC\n",
        )
        .cmp(
            &["replace", "-ir", r"_\d{3}", ".."],
            INPUT,
            ">id.. some desc\nATGC\n",
        )
        .cmp(
            &["replace", "-ir", r"_(\d{3})", "..$1"],
            INPUT,
            ">id..123 some desc\nATGC\n",
        );
}
