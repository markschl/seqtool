use crate::io::Record;

use bio::alphabets::{dna, protein, rna};
use clap::ValueEnum;
use strum_macros::{Display, EnumString};

use self::SeqType::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Display, EnumString, ValueEnum)]
pub enum SeqType {
    Dna,
    Rna,
    Protein,
    Other,
}

// For excluding certain characters when running recognition
fn filter_iter(text: &[u8]) -> impl Iterator<Item = &u8> {
    text.iter()
        .filter(|&s| !matches!(s, b'-' | b'.' | b'?' | b' '))
}

// returns (`SeqType`, has_wildcard (N/X), has_ambiguities(IUPAC))
// TODO: decide on exact behaviour
pub fn guess_seqtype(text: &[u8], hint: Option<SeqType>) -> Option<(SeqType, bool, bool)> {
    match hint {
        Some(SeqType::Dna) => Some(guess_dna(text).unwrap_or((SeqType::Dna, true, true))),
        Some(SeqType::Rna) => Some(guess_rna(text).unwrap_or((SeqType::Rna, true, true))),
        Some(SeqType::Protein) => {
            Some(guess_protein(text).unwrap_or((SeqType::Protein, true, true)))
        }
        Some(SeqType::Other) => Some((Other, false, false)),
        None => Some(
            guess_dna(text)
                .or_else(|| guess_rna(text))
                .or_else(|| guess_protein(text))
                .unwrap_or((Other, false, false)),
        ),
    }
}

pub fn guess_dna(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if dna::alphabet().is_word(filter_iter(text)) {
        Some((Dna, false, false))
    } else if dna::n_alphabet().is_word(filter_iter(text)) {
        Some((Dna, true, false))
    } else if dna::iupac_alphabet().is_word(filter_iter(text)) {
        Some((Dna, true, true))
    } else {
        None
    }
}

pub fn guess_rna(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if rna::alphabet().is_word(filter_iter(text)) {
        Some((Rna, false, false))
    } else if rna::n_alphabet().is_word(filter_iter(text)) {
        Some((Rna, true, false))
    } else if rna::iupac_alphabet().is_word(filter_iter(text)) {
        Some((Rna, true, true))
    } else {
        None
    }
}

pub fn guess_protein(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if protein::alphabet().is_word(filter_iter(text)) {
        Some((Protein, true, false))
    } else if filter_iter(text).any(|&b| (b as char).is_alphabetic()) {
        // all letters can potentially represent an amino acid or
        // an IUPAC ambiguity code
        Some((Protein, false, false))
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct SeqtypeHelper {
    seqtype: Option<SeqType>,
}

impl SeqtypeHelper {
    pub fn new(typehint: Option<SeqType>) -> Self {
        Self { seqtype: typehint }
    }

    pub fn get_or_guess(&mut self, record: &dyn Record) -> Result<SeqType, &'static str> {
        if let Some(seqtype) = self.seqtype {
            Ok(seqtype)
        } else {
            let mut buf = Vec::new();
            let seq = record.full_seq(&mut buf);
            let (seqtype, _, _) = guess_seqtype(&seq, self.seqtype)
                .ok_or_else(|| "Could not guess sequence type, please provide with `--seqtype`")?;
            self.seqtype = Some(seqtype);
            Ok(seqtype)
        }
    }
}
