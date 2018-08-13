use super::*;

#[test]
fn replace() {
    let fa = ">id_123 some desc\nA\nT\nGC\n";
    Tester::new()
        .cmp(&["replace", "T", "U"], fa, ">id_123 some desc\nAUGC\n")
        .cmp(&["replace", "ATG", "TGA"], fa, ">id_123 some desc\nTGAC\n")
        .cmp(&["replace", "-r", "[AT]", "?"], fa, ">id_123 some desc\n??GC\n")
        .cmp(&["replace", "-ir", r"_\d{3}", ".."], fa, ">id.. some desc\nATGC\n")
        .cmp(&["replace", "-ir", r"_(\d{3})", "..$1"], fa, ">id..123 some desc\nATGC\n")
        .cmp(&["replace", "-d", "e", "a"], fa, ">id_123 soma dasc\nATGC\n");
}
