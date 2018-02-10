
extern crate tempdir;
extern crate assert_cli;

#[allow(unused_imports)]
use std::io::{Read,Write};
use std::process::{Command,Stdio};
use std::env;
use std::fs::File;
use std::convert::AsRef;
use std::path::PathBuf;
use assert_cli::Assert;


trait Input {
    fn set(&self, a: Assert) -> Assert;
}

impl<T> Input for T where T: AsRef<str> {
    fn set(&self, a: Assert) -> Assert {
        a.stdin(self.as_ref())
    }
}

#[derive(Debug, Clone)]
struct FileInput<'a>(&'a str);

impl<'a> Input for FileInput<'a> {
    fn set(&self, a: Assert) -> Assert {
        a.with_args(&[self.0])
    }
}

#[derive(Debug, Clone)]
struct MultiFileInput(Vec<String>);

impl Input for MultiFileInput {
    fn set(&self, a: Assert) -> Assert {
        a.with_args(&self.0.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    }
}


struct Tester {
    root: PathBuf,
    bin: PathBuf
}

impl Tester {
    fn new() -> Tester {
        let mut a = Assert::command(&["cargo", "run"]);
        if cfg!(feature="exprtk") {
            a = a.with_args(&["--features=exprtk"]);
        }
        a.succeeds().execute().unwrap();

        // then return the path
        let root = Self::root();

        let name = "seqtool";
        let name = if cfg!(windows) {
                format!("{}.exe", name)
            } else {
                name.to_string()
            };

        Tester {
            bin: root.join(name),
            root: root,
        }
    }

    fn root() -> PathBuf {
        // from BurntSushi's xsv test code
        let mut root = env::current_exe()
            .unwrap()
            .parent()
            .expect("executable's directory")
            .to_path_buf();

        if root.ends_with("deps") {
            root.pop();
        }
        root
    }

    fn temp_dir<F, O>(&self, prefix: &str, mut f: F) -> O
        where F: FnMut(&mut tempdir::TempDir) -> O
    {
        let mut d = tempdir::TempDir::new_in(&self.root, prefix).expect("Could not create temp. dir");
        let out = f(&mut d);
        d.close().unwrap();
        out
    }

    fn temp_file<F, O>(&self, name: &str, content: Option<&str>, mut func: F) -> O
        where F: FnMut(&str, &mut File) -> O
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

    fn cmd<I: Input>(&self, args: &[&str], input: I) -> Assert {
        let a = Assert::command(&[self.bin.to_str().unwrap()])
            .with_args(args);
        input.set(a)
    }

    fn cmp<I: Input>(&self, args: &[&str], input: I, expected: &str) -> &Self {
        self.cmd(args, input)
            .stdout().is(expected)
            .execute().unwrap();
        self
    }

    fn succeeds<I: Input>(&self, args: &[&str], input: I) -> &Self {
        self.cmd(args, input)
            .succeeds()
            .execute().unwrap();
        self
    }

    fn fails<I: Input>(&self, args: &[&str], input: I, msg: &str) -> &Self {
        self.cmd(args, input)
            .fails()
            .stderr().contains(msg)
            .execute().unwrap();
        self
    }

    fn pipe(&self, args1: &[&str], input: &str, args2: &[&str], expected_out: &str) -> &Self {
        let p1 = Command::new(&self.bin)
            .args(args1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("could not run 1");
        p1.stdin.unwrap().write(input.as_bytes()).expect("write error");

        let p2 = Command::new(&self.bin)
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

// used by many tests:

static SEQS: [&'static str; 4] = [
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
    static ref __FASTA_STRING: String = SEQS.concat();
    #[derive(Eq, PartialEq, Debug)]
    static ref FASTA: &'static str = &__FASTA_STRING;
}

fn select_fasta(seqs: &[usize]) -> String {
    seqs.into_iter()
        .map(|i| SEQS[*i])
        .collect::<Vec<_>>()
        .concat()
}


mod pass;
mod compress;
mod convert;
mod count;
mod slice;
mod sample;
mod head;
mod tail;
mod trim;
mod set;
mod del;
mod replace;
mod find;
mod split;
mod upper;
mod lower;
mod mask;
mod revcomp;
mod stat;
#[cfg(feature = "exprtk")]
mod filter;
mod interleave;
