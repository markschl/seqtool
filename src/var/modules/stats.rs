use bytecount;

use var_provider::{DynVarProviderInfo, VarType, dyn_var_provider};
use variable_enum_macro::variable_enum;

use crate::io::{QualConverter, Record};
use crate::var::{VarBuilder, VarStore, attr::Attributes, parser::Arg, symbols::SymbolTable};

use super::VarProvider;

variable_enum! {
    /// # Sequence statistics
    ///
    ///
    /// # Examples
    ///
    /// List the GC content (in %) for every sequence
    ///
    /// `st stat gc_percent input.fa`
    ///
    /// seq1	33.3333
    /// seq2	47.2652
    /// seq3	47.3684
    ///
    ///
    /// Remove DNA sequences with more than 1% ambiguous bases
    ///
    /// `st filter 'charcount("ACGT") / seqlen >= 0.99' input.fa`
    StatVar {
        /// Sequence length
        Seqlen(Number),
        /// Ungapped sequence length (without gap characters `-`)
        UngappedSeqlen(Number),
        /// GC content as fraction (0-1) of total bases. Lowercase (=masked) letters or characters
        /// other than ACGTU are not taken into account.
        Gc(Number),
        /// GC content as percentage of total bases. Lowercase (=masked) letters or characters
        /// other than ACGTU are not taken into account.
        GcPercent(Number),
        /// Count the occurrences of one or more single characters, which are supplied as a string
        Charcount(Number) { characters: String },
        /// Total number of errors expected in the sequence, calculated from the quality scores
        /// as the sum of all error probabilities. For FASTQ, make sure to specify the correct
        /// format (--fmt) in case the scores are not in the Sanger/Illumina 1.8+ format.
        ExpErr(Number),
    }
}

#[derive(Debug, Default)]
pub struct StatVars {
    vars: VarStore<StatVar>,
}

impl StatVars {
    pub fn new() -> Self {
        Self::default()
    }
}

impl VarProvider for StatVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(StatVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(StatVar::from_func(name, args)?.map(|(var, out_type)| {
            let symbol_id = builder.store_register(var, &mut self.vars);
            (symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        !self.vars.is_empty()
    }

    fn set_record(
        &mut self,
        rec: &dyn Record,
        symbols: &mut SymbolTable,
        _: &Attributes,
        qual_converter: &mut QualConverter,
    ) -> Result<(), String> {
        for &(symbol_id, ref stat) in self.vars.iter() {
            let sym = symbols.get_mut(symbol_id).inner_mut();
            use StatVar::*;
            match stat {
                Seqlen => sym.set_int(rec.seq_len() as i64),
                Gc => sym.set_float(get_gc(rec.seq_segments())),
                GcPercent => sym.set_float(get_gc(rec.seq_segments()) * 100.),
                UngappedSeqlen => sym.set_int(get_ungapped_len(rec, b'-') as i64),
                Charcount { characters, .. } => {
                    let n = if characters.len() == 1 {
                        count_byte(rec, characters.as_bytes()[0]) as i64
                    } else {
                        count_bytes(rec, characters.as_bytes()) as i64
                    };
                    sym.set_int(n as i64)
                }
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

// TODO: efficient?
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
