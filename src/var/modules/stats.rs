use io::Record;
use error::CliResult;
use var::*;
use self::Stat::*;
use bytecount;

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
        ])
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Get absolute GC content (not relative to sequence length)",
                "seqtool stat count:GC input.fa",
            ),
        ])
    }
}

#[derive(Debug)]
enum Stat {
    SeqLen,
    UngappedLen,
    GC,
    Count(u8),
    MultiCount(Vec<u8>),
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
                SeqLen => data.symbols
                    .set_int(id, rec.seq_segments().fold(0, |l, s| l + s.len()) as i64),

                GC => {
                    let mut n = 0u64;
                    let mut gc = 0u64;
                    for seq in rec.seq_segments() {
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
                    data.symbols.set_float(id, gc as f64 / n as f64 * 100.)
                }

                UngappedLen => {
                    let n = rec.seq_segments()
                        .fold(0, |n, s| n + s.iter().filter(|&&c| c != b'-').count());
                    data.symbols.set_int(id, n as i64);
                }

                Count(byte) => {
                    let mut n = 0;
                    for seq in rec.seq_segments() {
                        n += bytecount::count(seq, byte);
                    }
                    data.symbols.set_int(id, n as i64);
                }

                MultiCount(ref bytes) => {
                    let mut n = 0;
                    for seq in rec.seq_segments() {
                        n += seq.iter()
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
                    data.symbols.set_int(id, n as i64);
                }
            }
        }
        Ok(())
    }
}
