use super::seqtype::SeqType;

pub fn reverse_complement<'a, S>(
    seq_iter: S,
    out: &mut Vec<u8>,
    seqtype: SeqType,
) -> Result<(), &'static str>
where
    S: Iterator<Item = &'a [u8]> + DoubleEndedIterator,
{
    let complement = match seqtype {
        SeqType::Dna => bio::alphabets::dna::complement,
        SeqType::Rna => bio::alphabets::rna::complement,
        _ => {
            return Err(
                "Only DNA/RNA sequences can be reverse-complemented, but the sequence type \
                is different. Wrongly recognized sequence types can be adjusted with `--seqtype`.",
            )
        }
    };
    out.clear();
    for s in seq_iter.rev() {
        out.extend(s.iter().rev().cloned().map(complement));
    }
    Ok(())
}
