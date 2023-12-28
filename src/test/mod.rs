use std::convert::AsRef;
use std::fs::File;
#[allow(unused_imports)]
use std::io::{Read, Write};
use std::process::{Command as StdCommand, Stdio};
use std::str;

use assert_cmd::{assert::Assert, cargo::cargo_bin, Command};
use fxhash::FxHashMap;
use itertools::Itertools;
use predicates::{ord::eq, prelude::*, str::contains};

trait Input {
    fn set<'a>(&mut self, a: &'a mut Command) -> &'a mut Command;
}

impl<T> Input for T
where
    T: AsRef<str>,
{
    fn set<'a>(&mut self, a: &'a mut Command) -> &'a mut Command {
        a.write_stdin(self.as_ref().as_bytes().to_owned())
    }
}

#[derive(Debug, Clone)]
struct FileInput<'a>(&'a str);

impl Input for FileInput<'_> {
    fn set<'a>(&mut self, a: &'a mut Command) -> &'a mut Command {
        a.args([self.0])
    }
}

#[derive(Debug, Clone)]
struct MultiFileInput(Vec<String>);

impl Input for MultiFileInput {
    fn set<'a>(&mut self, a: &'a mut Command) -> &'a mut Command {
        a.args(&self.0.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }
}

struct Tester {
    vars: FxHashMap<String, String>,
}

impl Tester {
    fn new() -> Tester {
        Tester {
            vars: FxHashMap::default(),
        }
    }

    fn temp_dir<F, O>(&self, prefix: &str, mut f: F) -> O
    where
        F: FnMut(&mut tempdir::TempDir) -> O,
    {
        let mut d = tempdir::TempDir::new(prefix).expect("Could not create temp. dir");
        let out = f(&mut d);
        d.close().unwrap();
        out
    }

    fn temp_file<F, O>(&self, name: &str, content: Option<&str>, mut func: F) -> O
    where
        F: FnMut(&str, &mut File) -> O,
    {
        self.temp_dir("test", |d| {
            let p = d.path().join(name);
            let mut f = File::create(&p).expect("Error creating file");
            if let Some(c) = content {
                f.write_all(c.as_bytes()).unwrap();
                f.flush().unwrap();
            }
            func(p.to_str().expect("invalid path name"), &mut f)
        })
    }

    fn cmd<I: Input>(&self, args: &[&str], mut input: I) -> Assert {
        let mut a = Command::cargo_bin("st").unwrap();
        a.args(args).envs(&self.vars);
        input.set(&mut a).assert()
    }

    fn cmp<I: Input>(&self, args: &[&str], input: I, expected: &str) -> &Self {
        self.cmd(args, input)
            .stdout(eq(expected).from_utf8())
            .success();
        self
    }

    fn succeeds<I: Input>(&self, args: &[&str], input: I) -> &Self {
        self.cmd(args, input).success();
        self
    }

    fn fails<I: Input>(&self, args: &[&str], input: I, msg: &str) -> &Self {
        self.cmd(args, input)
            .failure()
            .stderr(contains(msg).from_utf8());
        self
    }

    fn pipe(&self, args1: &[&str], input: &str, args2: &[&str], expected_out: &str) -> &Self {
        let p1 = StdCommand::new(cargo_bin("st"))
            .args(args1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("could not run 1");
        p1.stdin
            .unwrap()
            .write_all(input.as_bytes())
            .expect("write error");

        let p2 = StdCommand::new(cargo_bin("st"))
            .args(args2)
            .stdin(p1.stdout.unwrap())
            .output()
            .expect("could not run 2");

        assert_eq!(&String::from_utf8_lossy(&p2.stdout), expected_out);

        self
    }
}

fn fasta_record(seq: &str) -> String {
    format!(">seq \n{}\n", seq)
}

fn fq_records<Q1, Q2>(q1: Q1, q2: Q2) -> String
where
    Q1: AsRef<[u8]>,
    Q2: AsRef<[u8]>,
{
    let q1 = q1.as_ref();
    let q2 = q2.as_ref();
    format!(
        "@seq1\n{}\n+\n{}\n@seq2\n{}\n+\n{}\n",
        "A".repeat(q1.len()),
        str::from_utf8(q1).unwrap(),
        "G".repeat(q2.len()),
        str::from_utf8(q2).unwrap(),
    )
}

// used by many tests:

static SEQS: [&str; 4] = [
    ">seq1 p=2\nTTGGCAGGCCAAGGCCGATGGATCA\n",
    ">seq0 p=1\nCTGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq3 p=10\nCAGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq2 p=11\nACGG-AGGCC-AGGCCGATGGATCA\n",
];

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA (0) len=25, GC=0.6
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA (1) len=24, GC=0.667
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA (2) len=24, GC=0.667
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA (3) len=23, GC=0.652

lazy_static! {
    static ref __FASTA_STRING: String = SEQS.concat();
    #[derive(Eq, PartialEq, Debug)]
    static ref FASTA: &'static str = &__FASTA_STRING;
}

macro_rules! records {
    ($($i:expr),*) => {
        &[$($i),*].into_iter().map(|i| &SEQS[i]).join("")
    }
}

mod compress;
mod concat;
mod convert;
mod count;
mod del;
mod filter;
mod find;
mod head;
mod interleave;
mod lower;
mod mask;
mod pass;
mod replace;
mod revcomp;
mod sample;
mod set;
mod slice;
mod sort;
mod split;
mod stat;
mod tail;
mod trim;
mod unique;
mod upper;
