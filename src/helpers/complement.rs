use super::seqtype::SeqType;

/// Reverse complements a set of sequence chunks belonging to the same sequence
/// writes the contiguous reverse-complement to output
pub fn reverse_complement<'a, S>(
    seq_iter: S,
    out: &mut Vec<u8>,
    seqtype: SeqType,
) -> Result<(), String>
where
    S: Iterator<Item = &'a [u8]> + DoubleEndedIterator,
{
    let complement = match seqtype {
        SeqType::DNA => bio::alphabets::dna::complement,
        SeqType::RNA => bio::alphabets::rna::complement,
        _ => {
            return Err(format!(
                "Only DNA/RNA sequences can be reverse-complemented, but the sequence type \
                is '{seqtype}'. Wrongly recognized sequence types can be adjusted with `--seqtype`."
            ));
        }
    };
    out.clear();
    for s in seq_iter.rev() {
        out.extend(s.iter().rev().cloned().map(complement));
    }
    Ok(())
}
