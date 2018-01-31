
use super::*;


#[test]
fn lower() {
    let fa = ">seq\naTgC\n";
    Tester::new()
        .cmp(&["lower"], fa, ">seq\natgc\n");
}
