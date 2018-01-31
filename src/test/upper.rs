
use super::*;


#[test]
fn upper() {
    let fa = ">seq\naTgC\n";
    Tester::new()
        .cmp(&["upper"], fa, ">seq\nATGC\n");
}
