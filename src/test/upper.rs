use super::*;

#[test]
fn upper() {
    let fa = ">seq\naTgC\n";
    cmp(&["upper"], fa, ">seq\nATGC\n");
}
