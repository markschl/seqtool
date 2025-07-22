use std::convert::AsRef;
use std::fs::{read_to_string, remove_file};
use std::io::{self, Read, Write};
use std::ops::Deref;
use std::process::{Command as StdCommand, Stdio};
use std::str;
use std::sync::LazyLock;

use assert_cmd::{assert::Assert, cargo::cargo_bin, Command};
use itertools::Itertools;
use predicates::{ord::eq, prelude::*, str::contains};

fn with_tmpdir<F, O>(prefix: &str, f: F) -> O
where
    F: FnOnce(TempDir) -> O,
{
    let td = TempDir::new(prefix);
    f(td)
}

struct TempDir(tempfile::TempDir);

impl TempDir {
    fn new(prefix: &str) -> Self {
        Self(tempfile::TempDir::with_prefix(prefix).unwrap())
    }

    fn persistent_path(&self, fname: &str) -> String {
        self.0.path().join(fname).to_str().unwrap().to_string()
    }

    fn path(&self, fname: &str) -> TempPath {
        TempPath(self.persistent_path(fname))
    }

    fn file(&self, ext: &str, content: &str) -> TempFile {
        let mut f = tempfile::NamedTempFile::with_suffix_in(ext, self.0.path()).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        TempFile(f)
    }

    fn multi_file<I, S>(&self, ext: &str, content: I) -> Vec<TempFile>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        content
            .into_iter()
            .map(|s| self.file(ext, s.as_ref()))
            .collect()
    }
}

// impl Deref for TempDir {
//     type Target = tempfile::TempDir;

//     fn deref(&self) -> &tempfile::TempDir {
//         &self.0
//     }
// }

fn tmp_file(prefix: &str, ext: &str, content: &str) -> TempFile {
    let mut f = tempfile::Builder::new()
        .prefix(prefix)
        .suffix(ext)
        .tempfile()
        .unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    TempFile(f)
}

struct TempFile(tempfile::NamedTempFile);

impl TempFile {
    fn path_str(&self) -> &str {
        self.0.path().to_str().unwrap()
    }
}

impl io::Write for TempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Deref for TempFile {
    type Target = str;

    fn deref(&self) -> &str {
        self.path_str()
    }
}

/// Represents a path in the temporary directory, which can be used for output files
#[derive(Debug)]
struct TempPath(String);

impl TempPath {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn content(&self) -> String {
        read_to_string(&self.0).unwrap()
    }

    #[cfg(feature = "gz")]
    fn gz_content(&self) -> String {
        use std::fs::File;

        let mut s = String::new();
        flate2::read::MultiGzDecoder::new(File::open(&self.0).unwrap())
            .read_to_string(&mut s)
            .unwrap();
        s
    }
}

impl Deref for TempPath {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        remove_file(&self.0).ok();
    }
}

trait Input {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command;
}

impl Input for &str {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        a.write_stdin(self.as_bytes().to_owned())
    }
}

impl Input for String {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        (self.as_str()).apply_to(a)
    }
}

impl Input for &String {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        (self.as_str()).apply_to(a)
    }
}

impl Input for TempFile {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        a.args([self.path_str()])
    }
}

impl Input for &TempFile {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        (*self).apply_to(a)
    }
}

impl Input for &[TempFile] {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        a.args(self.iter().map(|f| f.path_str()))
    }
}

impl Input for Vec<TempFile> {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        self.as_slice().apply_to(a)
    }
}

impl Input for &Vec<TempFile> {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        self.as_slice().apply_to(a)
    }
}

impl Input for &TempPath {
    fn apply_to<'a>(&self, a: &'a mut Command) -> &'a mut Command {
        a.args([&self.0])
    }
}

fn cmd<I: Input>(args: &[&str], input: I) -> Assert {
    let mut c = Command::cargo_bin("st").unwrap();
    c.args(args);
    input.apply_to(&mut c).assert()
}

fn cmp<I: Input>(args: &[&str], input: I, expected: &str) {
    cmd(args, input).stdout(eq(expected).from_utf8()).success();
}

fn cmp_stderr<I: Input>(args: &[&str], input: I, expected: &str, stderr: &str) {
    cmd(args, input)
        .stdout(eq(expected).from_utf8())
        .stderr(contains(stderr).from_utf8())
        .success();
}

fn succeeds<I: Input>(args: &[&str], input: I) {
    cmd(args, input).success();
}

fn fails<I: Input>(args: &[&str], input: I, msg: &str) {
    cmd(args, input).failure().stderr(contains(msg).from_utf8());
}

fn cmp_pipe(args1: &[&str], input: &str, args2: &[&str], expected_out: &str) {
    let mut p1 = StdCommand::new(cargo_bin("st"))
        .args(args1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("could not run 1");
    p1.stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .expect("write error");

    let p2 = StdCommand::new(cargo_bin("st"))
        .args(args2)
        .stdin(p1.stdout.take().unwrap())
        .output()
        .expect("could not run 2");

    assert!(p1.wait().unwrap().success());

    assert_eq!(&String::from_utf8_lossy(&p2.stdout), expected_out);
}

fn cmd_with_env<I, E>(args: &[&str], input: I, env: E) -> Assert
where
    I: Input,
    E: IntoIterator<Item = (&'static str, &'static str)>,
{
    let mut c = Command::cargo_bin("st").unwrap();
    c.args(args).envs(env);
    input.apply_to(&mut c).assert()
}

fn cmp_with_env<I, E>(args: &[&str], input: I, expected: &str, env: E)
where
    I: Input,
    E: IntoIterator<Item = (&'static str, &'static str)>,
{
    cmd_with_env(args, input, env)
        .stdout(eq(expected).from_utf8())
        .success();
}

fn fasta_record(seq: &str) -> String {
    format!(">seq \n{seq}\n")
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

const SEQS: [&str; 4] = [
    ">seq1 p=2\nTTGGCAGGCCAAGGCCGATGGATCA\n",
    ">seq0 p=1\nCTGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq3 p=10\nCAGGCAGGCC-AGGCCGATGGATCA\n",
    ">seq2 p=11\nACGG-AGGCC-AGGCCGATGGATCA\n",
];

// >seq1 p=2
// TTGGCAGGCCAAGGCCGATGGATCA
// >seq0 p=1
// CTGGCAGGCC-AGGCCGATGGATCA
// >seq3 p=10
// CAGGCAGGCC-AGGCCGATGGATCA
// >seq2 p=11
// ACGG-AGGCC-AGGCCGATGGATCA

// id	desc	seq
// seq1	p=2	    TTGGCAGGCCAAGGCCGATGGATCA (0) len=25, GC=0.6
// seq0	p=1	    CTGGCAGGCC-AGGCCGATGGATCA (1) len=24, GC=0.667
// seq3	p=10	CAGGCAGGCC-AGGCCGATGGATCA (2) len=24, GC=0.667
// seq2	p=11	ACGG-AGGCC-AGGCCGATGGATCA (3) len=23, GC=0.652

static FASTA: LazyLock<String> = LazyLock::new(|| SEQS.concat());

macro_rules! records {
    ($($i:expr),*) => {
        &[$($i),*].into_iter().map(|i| &SEQS[i]).join("")
    }
}

#[cfg(any(feature = "all-commands", feature = "cmp"))]
mod cmp;
#[cfg(any(feature = "gz", feature = "lz4"))]
mod compress;
#[cfg(any(feature = "all-commands", feature = "concat"))]
mod concat;
#[cfg(any(feature = "all-commands", feature = "pass"))]
mod convert;
#[cfg(any(feature = "all-commands", feature = "count"))]
mod count;
#[cfg(any(feature = "all-commands", feature = "del"))]
mod del;
#[cfg(any(
    all(feature = "expr", feature = "all-commands"),
    all(feature = "expr", feature = "filter")
))]
mod filter;
#[cfg(any(feature = "all-commands", feature = "find"))]
mod find;
#[cfg(any(feature = "all-commands", feature = "head"))]
mod head;
#[cfg(any(feature = "all-commands", feature = "interleave"))]
mod interleave;
#[cfg(any(feature = "all-commands", feature = "lower"))]
mod lower;
#[cfg(any(feature = "all-commands", feature = "mask"))]
mod mask;
#[cfg(any(feature = "all-commands", feature = "pass"))]
mod pass;
#[cfg(any(feature = "all-commands", feature = "replace"))]
mod replace;
#[cfg(any(feature = "all-commands", feature = "revcomp"))]
mod revcomp;
#[cfg(any(feature = "all-commands", feature = "sample"))]
mod sample;
#[cfg(any(feature = "all-commands", feature = "set"))]
mod set;
#[cfg(any(feature = "all-commands", feature = "slice"))]
mod slice;
#[cfg(any(feature = "all-commands", feature = "sort"))]
mod sort;
#[cfg(any(feature = "all-commands", feature = "split"))]
mod split;
#[cfg(any(feature = "all-commands", feature = "stat"))]
mod stat;
#[cfg(any(feature = "all-commands", feature = "tail"))]
mod tail;
#[cfg(any(feature = "all-commands", feature = "trim"))]
mod trim;
#[cfg(any(feature = "all-commands", feature = "unique"))]
mod unique;
#[cfg(any(feature = "all-commands", feature = "upper"))]
mod upper;
#[cfg(any(feature = "all-commands", feature = "pass"))]
mod vars;
