extern crate tempdir;

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
