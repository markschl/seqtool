
use super::*;



#[test]
fn stats() {
    let seq = ">seq\nATGC-NYA\n";
    let retval = "seq\t8\t7\t40\t2\t3";
    let vars = "s:seqlen,s:ungapped_len,s:gc,s:count:A,s:count:AT";
    let vars_noprefix = vars.replace("s:", "");
    let retval2 = format!("id\t{}\n{}", vars_noprefix.replace(",", "\t"), retval);
    Tester::new()
        .cmp(&[".", "--to-txt", &format!("id,{}", vars)], seq, retval)
        .cmp(&["stat", &vars_noprefix], seq, &retval2);
}
