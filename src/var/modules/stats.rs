use self::Stat::*;
use bytecount;
use error::CliResult;
use io::Record;
use var::*;

pub struct StatHelp;

impl VarHelp for StatHelp {
    fn name(&self) -> &'static str {
        "Sequence statistics"
    }
    fn usage(&self) -> &'static str {
        "s:<variable>[:opts]"
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            ("s:seqlen", "Sequence length"),
            ("s:ungapped_len", "Sequence length without gaps (-)"),
            (
                "s:gc",
                "GC content as percentage of total bases. Lowercase (=masked) letters \
                 / characters other than ACGTU are not counted.",
            ),
            (
                "s:count",
                "Count occurrence one or more characters. Usage: `s:count:<characters>`. \
                 Note that some characters (like '-') cannot be specified in math expressions.",
            ),
            (
                "s:exp_err",
                "Expected errors for the whole sequence calculated based on quality scores \
                 as the sum of all error probabilities.",
            ),
        ])
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "Get absolute GC content (not relative to sequence length)",
            "st stat count:GC input.fa",
        )])
    }
}

#[derive(Debug)]
enum Stat {
    SeqLen,
    UngappedLen,
    GC,
    Count(u8),
    MultiCount(Vec<u8>),
    ExpErr,
}

#[derive(Debug)]
pub struct StatVars {
    stats: Vec<(Stat, usize)>,
}

impl StatVars {
    pub fn new() -> StatVars {
        StatVars { stats: vec![] }
    }
}

impl VarProvider for StatVars {
    fn prefix(&self) -> Option<&str> {
        Some("s")
    }

    fn name(&self) -> &'static str {
        "statistics"
    }

    fn register_var(&mut self, name: &str, id: usize, _: &mut VarStore) -> CliResult<bool> {
        let parts: Vec<_> = name.splitn(2, ':').collect();
        let args = parts.get(1);
        let name = parts[0];
        let stat = match name {
            "seqlen" => SeqLen,
            "ungapped_len" => UngappedLen,
            "gc" => GC,
            "count" => {
                if let Some(c) = args {
                    let c = c.as_bytes();
                    if c.len() == 1 {
                        Stat::Count(c[0])
                    } else {
                        Stat::MultiCount(c.to_owned())
                    }
                } else {
                    return fail!("Please specify one or more characters to count.");
                }
            }
            "exp_err" => ExpErr,
            _ => return Ok(false),
        };
        self.stats.push((stat, id));
        Ok(true)
    }

    fn has_vars(&self) -> bool {
        !self.stats.is_empty()
    }

    fn set(&mut self, rec: &Record, data: &mut Data) -> CliResult<()> {
        for &(ref stat, id) in &self.stats {
            match *stat {
                SeqLen => data.symbols.set_int(id, rec.seq_len() as i64),

                GC => data
                    .symbols
                    .set_float(id, get_gc(rec.seq_segments()) * 100.),

                UngappedLen => data.symbols.set_int(id, get_ungapped_len(rec, b'-') as i64),

                Count(byte) => data.symbols.set_int(id, count_byte(rec, byte) as i64),

                MultiCount(ref bytes) => data.symbols.set_int(id, count_bytes(rec, bytes) as i64),

                ExpErr => {
                    let q = rec.qual().ok_or("No quality scores in input.")?;
                    data.symbols.set_float(id, data.qual_converter.prob_sum(q)?);
                }
            }
        }
        Ok(())
    }
}

#[inline]
fn get_ungapped_len<R: Record>(rec: R, gap_char: u8) -> usize {
    rec.seq_segments()
        .fold(0, |n, s| n + s.iter().filter(|&&c| c != gap_char).count())
}

#[inline]
fn count_byte<R: Record>(rec: R, byte: u8) -> usize {
    let mut n = 0;
    for seq in rec.seq_segments() {
        n += bytecount::count(seq, byte);
    }
    n
}

#[inline]
fn count_bytes<R: Record>(rec: R, bytes: &[u8]) -> usize {
    let mut n = 0;
    for seq in rec.seq_segments() {
        n += seq
            .iter()
            .filter(|&b| {
                for b0 in bytes {
                    if b == b0 {
                        return true;
                    }
                }
                false
            })
            .count();
    }
    n
}

#[inline]
fn get_gc<'a, I>(seqs: I) -> f64
where
    I: Iterator<Item = &'a [u8]>,
{
    let mut n = 0u64;
    let mut gc = 0u64;
    for seq in seqs {
        for b in seq {
            match *b {
                b'C' | b'G' => {
                    n += 1;
                    gc += 1;
                }
                b'A' | b'T' | b'U' => {
                    n += 1;
                }
                _ => {}
            }
        }
    }
    gc as f64 / n as f64
}
