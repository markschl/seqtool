
use super::*;


#[test]
fn tail() {

    let t = Tester::new();
    t.fails(&["tail", "-n", "3"], *FASTA, "Cannot use STDIN as input");
    t.temp_file("tail", |p, f| {
        let path = p.to_str().unwrap();
        f.write_all(FASTA.as_bytes()).unwrap();
        f.flush().unwrap();
        t.cmp(&["tail", "-n", "2"], FileInput(path), &select_fasta(&[2, 3]));
    });
}
