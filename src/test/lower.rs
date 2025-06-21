use super::*;

#[test]
fn lower() {
    let fa = ">seq\naTgC\n";
    cmp(&["lower"], fa, ">seq\natgc\n");
}
