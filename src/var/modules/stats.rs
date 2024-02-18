use bytecount;

use self::Stat::*;
use crate::error::CliResult;
use crate::io::{QualConverter, Record};
use crate::var::{
    attr::Attrs,
    func::Func,
    symbols::{SymbolTable, VarType},
    VarBuilder, VarInfo, VarProvider, VarProviderInfo,
};
use crate::var_info;

#[derive(Debug)]
pub struct StatHelp;

impl VarProviderInfo for StatHelp {
    fn name(&self) -> &'static str {
        "Sequence statistics"
    }

    fn vars(&self) -> &[VarInfo] {
        &[
            var_info!(seqlen => "Sequence length"),
            var_info!(ungapped_seqlen => "Ungapped sequence length (without gap characters `-`)"),
            var_info!(
                gc =>
                "GC content as percentage of total bases. Lowercase (=masked) letters \
                 or characters other than ACGTU are not taken into account."
            ),
            var_info!(
                charcount (characters) =>
                "Count occurrence one or more characters."
            ),
            var_info!(
                exp_err =>
                "Total number of errors expected in the sequence, calculated from the quality scores \
                 as the sum of all error probabilities. For FASTQ, make sure to specify the correct \
                 format (--fmt) in case the scores are not in the Sanger/Illumina 1.8+ format."
            ),
        ]
    }
    fn examples(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[
            (
                "Get absolute GC content (not relative to sequence length)",
                "st stat gc input.fa",
            ),
            (
                "Remove DNA sequences with at least 1% ambiguous bases",
                "st filter 'charcount(\"ACGT\") / seqlen >= 0.01' input.fa",
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
    fn info(&self) -> &dyn VarProviderInfo {
        &StatHelp
    }

    fn register(&mut self, func: &Func, b: &mut VarBuilder) -> CliResult<Option<VarType>> {
        let name = func.name.as_str();
        let (vt, stat) = if name == "charcount" {
            let v = func.arg_as::<String>(0)?;
            let c = v.as_bytes();
            if c.len() == 1 {
                (VarType::Int, Stat::Count(c[0]))
            } else {
                (VarType::Int, Stat::MultiCount(c.to_owned()))
            }
        } else {
            match name {
                "seqlen" => (VarType::Int, SeqLen),
                "ungapped_seqlen" => (VarType::Int, UngappedLen),
                "gc" => (VarType::Float, GC),
                "exp_err" => (VarType::Float, ExpErr),
                _ => return Ok(None),
            }
        };
        self.stats.push((stat, b.symbol_id()));
        Ok(Some(vt))
    }

    fn has_vars(&self) -> bool {
        !self.stats.is_empty()
    }

    fn set(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        _: &mut Attrs,
        qual_converter: &mut QualConverter,
    ) -> CliResult<()> {
        for &(ref stat, id) in &self.stats {
            let sym = symbols.get_mut(id).inner_mut();
            match *stat {
                SeqLen => sym.set_int(rec.seq_len() as i64),
                GC => sym.set_float(get_gc(rec.seq_segments()) * 100.),
                UngappedLen => sym.set_int(get_ungapped_len(rec, b'-') as i64),
                Count(byte) => sym.set_int(count_byte(rec, byte) as i64),
                MultiCount(ref bytes) => sym.set_int(count_bytes(rec, bytes) as i64),
                ExpErr => {
                    let q = rec.qual().ok_or("No quality scores in input.")?;
                    let ee = qual_converter.total_error(q)?;
                    sym.set_float(ee);
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
