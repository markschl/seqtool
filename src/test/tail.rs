use super::*;

#[test]
fn tail() {
    fails(&["tail", "-n", "3"], &*FASTA, "Cannot use STDIN as input");
    let input = tmp_file("st_tail_", ".fasta", &FASTA);
    cmp(&["tail", "-n", "2"], input, records!(2, 3));
}
