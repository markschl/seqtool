extern crate tempdir;

#[allow(unused_imports)]
use std::io::{Read,Write};
use std::process::{Command,Stdio};

macro_rules! run {
    ($args:expr, $input:expr) => {
        Assert::main_binary().with_args($args)
            .stdin($input.as_ref())
     };
}

macro_rules! cmp_stdout {
    ($args:expr, $input:expr, $cmp:expr) => {
        run!($args, $input).stdout().is($cmp.as_ref()).unwrap();
     };
}

macro_rules! fails {
    ($args:expr, $input:expr, $msg:expr) => {
        Assert::main_binary().with_args($args)
            .stdin($input.as_ref())
            .fails()
            .stderr().contains($msg.as_ref()).unwrap();
     };
}

macro_rules! piped {
    ($args1:expr, $input:expr, $args2:expr) => {{
        let p1 = Command::new("cargo")
            .args(["run", "-q", "--"].into_iter().chain($args1))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("could not run 1");
        p1.stdin.unwrap().write($input.as_bytes()).expect("write error");

        let p2 = Command::new("cargo")
            .args(["run", "-q", "--"].into_iter().chain($args2))
            .stdin(p1.stdout.unwrap())
            .output()
            .expect("could not run 2");

        String::from_utf8_lossy(&p2.stdout).to_string()
    }};
}


static _SEQS: [&'static str; 4] = [
    ">seq1 p=2\nTTGGCAGGCCAAGGCCGATGGATCA\n",
    ">seq0 p=1\nCTGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq3 p=10\nCAGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq2 p=11\nACGG-AGGCC-AGGCCGATGGATCA\n",
];

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA	(0)
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA	(1)
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA	(2)
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA	(3)

lazy_static! {
    #[derive(Eq, PartialEq, Debug)]
    static ref FASTA: String = SEQS.concat();
    static ref SEQS: Vec<&'static str> = _SEQS.to_vec();
}

fn select(seqs: &[usize]) -> String {
    seqs.into_iter()
        .map(|i| _SEQS[*i])
        .collect::<Vec<_>>()
        .concat()
}

fn fasta_record(seq: &str) -> String {
    format!(">seq \n{}\n", seq)
}

mod tests;
mod test_find;
