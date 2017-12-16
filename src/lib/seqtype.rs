#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum SeqType {
    DNA,
    RNA,
    Protein,
    Other,
}

use bio::alphabets::{dna, protein, rna};
use self::SeqType::*;

// returns (`SeqType`, has_wildcard (N/X), has_ambiguities(IUPAC))
pub fn guess_seqtype(text: &[u8], hint: Option<&str>) -> Option<(SeqType, bool, bool)> {
    match hint {
        Some("dna") => guess_dna(text),
        Some("rna") => guess_rna(text),
        Some("protein") => guess_protein(text),
        None => Some(
            guess_dna(text)
                .or_else(|| guess_rna(text))
                .or_else(|| guess_protein(text))
                .unwrap_or((Other, false, false)),
        ),
        _ => None,
    }
}

pub fn guess_dna(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if dna::alphabet().is_word(text) {
        Some((DNA, false, false))
    } else if dna::n_alphabet().is_word(text) {
        Some((DNA, true, false))
    } else if dna::iupac_alphabet().is_word(text) {
        Some((DNA, true, true))
    } else {
        None
    }
}

pub fn guess_rna(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if rna::alphabet().is_word(text) {
        Some((RNA, false, false))
    } else if rna::n_alphabet().is_word(text) {
        Some((RNA, true, false))
    } else if rna::iupac_alphabet().is_word(text) {
        Some((RNA, true, true))
    } else {
        None
    }
}

pub fn guess_protein(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if protein::alphabet().is_word(text) {
        if text.iter().any(|a| *a == b'X' || *a == b'x') {
            Some((Protein, true, false))
        } else {
            Some((Protein, false, false))
        }
    } else {
        None
    }
}
