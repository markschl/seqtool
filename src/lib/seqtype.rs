use self::SeqType::*;
use bio::alphabets::{dna, protein, rna, Alphabet};

// TODO: maybe use lazy_static to initialize all alphabets. However, these
// function are rarely called...

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum SeqType {
    DNA,
    RNA,
    Protein,
    Other,
}

// For exclusing certain characters when running recognition
fn filter_iter<'a>(text: &'a [u8]) -> impl Iterator<Item = &'a u8> {
    text.into_iter().filter(|&s| match s {
        b'-' | b'.' | b'?' | b' ' => false,
        _ => true,
    })
}

// returns (`SeqType`, has_wildcard (N/X), has_ambiguities(IUPAC))
pub fn guess_seqtype(text: &[u8], hint: Option<&str>) -> Option<(SeqType, bool, bool)> {
    match hint {
        Some("dna") => guess_dna(text),
        Some("rna") => guess_rna(text),
        Some("protein") => guess_protein(text),
        Some("other") => Some((Other, false, false)),
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
    if dna::alphabet().is_word(filter_iter(text)) {
        Some((DNA, false, false))
    } else if dna::n_alphabet().is_word(filter_iter(text)) {
        Some((DNA, true, false))
    } else if dna::iupac_alphabet().is_word(filter_iter(text)) {
        Some((DNA, true, true))
    } else {
        None
    }
}

pub fn guess_rna(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    if rna::alphabet().is_word(filter_iter(text)) {
        Some((RNA, false, false))
    } else if rna::n_alphabet().is_word(filter_iter(text)) {
        Some((RNA, true, false))
    } else if rna::iupac_alphabet().is_word(filter_iter(text)) {
        Some((RNA, true, true))
    } else {
        None
    }
}

pub fn guess_protein(text: &[u8]) -> Option<(SeqType, bool, bool)> {
    let protein_x = Alphabet::new(&b"ARNDCEQGHILKMFPSTWYVXarndceqghilkmfpstwyvx"[..]);
    if protein_x.is_word(filter_iter(text)) {
        Some((Protein, true, false))
    } else if protein::alphabet().is_word(filter_iter(text)) {
        Some((Protein, false, false))
    } else {
        None
    }
}
