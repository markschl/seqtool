use crate::io::Record;

use bio::alphabets::{dna, protein, rna};
use clap::ValueEnum;
use strum_macros::{Display, EnumString};

use SeqType::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Display, EnumString, ValueEnum)]
pub enum SeqType {
    #[allow(clippy::upper_case_acronyms)]
    DNA,
    #[allow(clippy::upper_case_acronyms)]
    RNA,
    Protein,
    Other,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct SeqTypeInfo {
    pub seqtype: SeqType,
    /// has DNA/RNA or protein wildcards (N/X)
    pub has_wildcard: bool,
    /// has IUPAC ambiguities
    pub has_ambiguities: bool,
}

impl SeqTypeInfo {
    pub fn new(ty: SeqType, has_wildcard: bool, has_ambiguities: bool) -> Self {
        Self {
            seqtype: ty,
            has_ambiguities,
            has_wildcard,
        }
    }
}

// For excluding certain characters when running recognition
fn filter_iter(text: &[u8]) -> impl Iterator<Item = &u8> {
    text.iter()
        .filter(|&s| !matches!(s, b'-' | b'.' | b'?' | b' '))
}

/// Returns information about the sequence type. In case a type hint is provided,
/// the sequence is still checked for ambiguities and wildcards.
/// Returns Err(typehint) if the type hint does not match the actual sequence.
pub fn guess_seqtype(text: &[u8], hint: Option<SeqType>) -> Result<SeqTypeInfo, SeqType> {
    match hint {
        Some(DNA) => guess_dna(text).ok_or(DNA),
        Some(RNA) => guess_rna(text).ok_or(RNA),
        Some(Protein) => guess_protein(text).ok_or(Protein),
        Some(Other) => Ok(SeqTypeInfo::new(Other, false, false)),
        None => Ok(guess_dna(text)
            .or_else(|| guess_rna(text))
            .or_else(|| guess_protein(text))
            .unwrap_or(SeqTypeInfo::new(Other, false, false))),
    }
}

pub fn guess_seqtype_or_fail(
    text: &[u8],
    hint: Option<SeqType>,
    allow_other: bool,
) -> Result<SeqTypeInfo, String> {
    let info = guess_seqtype(text, hint).map_err(|hint| {
        format!(
            "The sequence type '{hint}' provided with `--seqtype` does not appear to be valid \
            for the given sequence. Please make sure that only valid characters are used and \
            note that only standard ambiguities according to IUPAC are recognized \
            (e.g. see https://bioinformatics.org/sms/iupac.html)."
        )
    })?;
    if !allow_other && info.seqtype == Other {
        return Err("Could not guess sequence type, please provide with `--seqtype`".to_string());
    }
    Ok(info)
}

pub fn guess_dna(text: &[u8]) -> Option<SeqTypeInfo> {
    if dna::alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(DNA, false, false))
    } else if dna::n_alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(DNA, true, false))
    } else if dna::iupac_alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(DNA, true, true))
    } else {
        None
    }
}

pub fn guess_rna(text: &[u8]) -> Option<SeqTypeInfo> {
    if rna::alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(RNA, false, false))
    } else if rna::n_alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(RNA, true, false))
    } else if rna::iupac_alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(RNA, true, true))
    } else {
        None
    }
}

pub fn guess_protein(text: &[u8]) -> Option<SeqTypeInfo> {
    if protein::alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(Protein, false, false))
    } else if protein::iupac_alphabet().is_word(filter_iter(text)) {
        Some(SeqTypeInfo::new(Protein, true, true))
    } else {
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct SeqtypeHelper {
    seqtype: Option<SeqType>,
}

impl SeqtypeHelper {
    pub fn new(typehint: Option<SeqType>) -> Self {
        Self { seqtype: typehint }
    }

    pub fn get_or_guess(&mut self, record: &dyn Record) -> Result<SeqType, String> {
        if let Some(seqtype) = self.seqtype {
            Ok(seqtype)
        } else {
            let mut buf = Vec::new();
            let seq = record.full_seq(&mut buf);
            let info = guess_seqtype_or_fail(&seq, self.seqtype, false)?;
            self.seqtype = Some(info.seqtype);
            Ok(info.seqtype)
        }
    }
}
