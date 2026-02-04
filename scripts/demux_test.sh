#!/usr/bin/env bash
# This script compares de-multiplexing and primer trimming behaviour
# of different tools, namely Seqtool, Cutadapt and USEARCH
# using a test dataset of 14k reads

set -euo pipefail

out=demux
rm -Rf $out
mkdir -p $out


################################################
#### 1. Required software
################################################

cargo build --release
st=target/release/st

# conda create -y -n demux_test cutadapt
# conda activate demux_test
if ! cutadapt --version; then
  echo 'Cutadapt not in $PATH'
fi

if ! usearch11; then
  echo 'USEARCH v11 not in $PATH (download from https://drive5.com/usearch/download.html)'
fi


################################################
#### 2. Download the data from this tutorial
#### https://drive5.com/usearch/manual/upp_tut_hmp.html
################################################

if [ ! -d hmptut ]; then
    wget https://drive5.com/downloads/hmptut_v10.tar.gz

    tar -xzf hmptut_v10.tar.gz
    rm hmptut_v10.tar.gz
fi

fq=reads.filter.fq
$st filter 'charcount("ATGC") == seqlen' --fq hmptut/fq/reads.fq > $fq

barcodes=hmptut/scripts/barcodes.fa
bc_len=$($st stat seqlen hmptut/scripts/barcodes.fa | cut -f2 | uniq)

# # create a larger sequence set with reshuffled reads (as they are in order of samples in the file)
# # (using SeqKit for that, Seqtool cannot do it)
# fq=hmptut/fq/reads_shuffled.fq
# echo -n > $fq
# for i in {1..20}; do
#     seqkit shuffle hmptut/fq/reads.fq | st filter 'charcount("ATGC") == seqlen' --fq >> $fq
# done


################################################
#### 3. Demultiplexing
################################################

diffs=2

st_demux() {
    hit=$1 && shift
    /usr/bin/time -f '%e' $st find file:$barcodes $fq \
      --filter \
      -D $diffs \
      --seqtype dna \
        -a sample={pattern_name} \
        -a bc="{match($hit)}" -a am="{aligned_match($hit)}" -a ap="{aligned_pattern($hit)}" \
        -a end="{match_end($hit)}" "$@" |
      $st trim -e '{attr(end)}:' --fq
}

cutadapt_demux() {
    /usr/bin/time -f '%e' cutadapt -e $diffs \
      --discard-untrimmed \
      --no-index \
      --rename '{id} sample={adapter_name} bc={match_sequence}' \
      --quiet \
      "$@" \
      $fq
}

usearch_demux() {
    /usr/bin/time -f '%e' usearch11 -fastx_demux $fq \
      -barcodes $barcodes \
      -fastqout /dev/stdout \
      -maxdiffs $diffs \
      -quiet \
      "$@" |
      $st replace -i ';' ' ' --fq |
      $st trim -e "$bc_len:" --fq
}

# gapped + anchored search at the start
st_demux 1 --anchor-start 0 > $out/st_demux.fq
st_demux all --anchor-start 0 > $out/st_demux_all.fq  # multiple hits -> different code path used in seqtool
cutadapt_demux -g ^file:$barcodes > $out/cutadapt_demux.fq

# ungapped search (not possible with seqtool)
cutadapt_demux -g ^file:$barcodes --no-indels > $out/cutadapt_demux_noindels.fq
usearch_demux > $out/usearch_demux.fq

st count -k filename $out/*.fq

# gapped + anchored search at the start
$st cmp -ck 'id,attr(sample),attr(bc),seqhash' $out/st_demux.fq $out/cutadapt_demux.fq
$st cmp -ck 'id,attr(sample),attr(bc),seqhash' $out/st_demux.fq $out/st_demux_all.fq

# hamming distance of 2
$st cmp -ck 'id,attr(sample),seqhash' $out/cutadapt_demux_noindels.fq $out/usearch_demux.fq


################################################
#### 3. Primer search
################################################

primer=CCGTCAATTCMTTTRAGT
primer_rc=$(st revcomp --csv id,seq --outfields seq <<< id,$primer)
demux=$out/cutadapt_demux.fq
demux_rev=$out/cutadapt_demux_rev.fq

# simulated reverse reads
st revcomp $demux > $demux_rev

err_rate=0.2
overlap=14

st_find_primer() {
    /usr/bin/time -f '%e' $st find -f \
       -R $err_rate --in-order \
       --seqtype dna \
      -a primer={match} \
      -a pstart={match_start} -a pend={match_end} \
      -a last_pstart='{match_start(-1)}' -a last_pend='{match_end(-1)}' \
      -a rngs='{match_range(all)}' -a neg_rngs='{match_neg_range(all)}' \
      -a pdiffs={match_diffs} \
      --to-fa \
      "$@"
}


cutadapt_trim_primer() {
    /usr/bin/time -f '%e' cutadapt \
      -e $err_rate \
      --overlap $overlap \
      --discard-untrimmed \
      --rename '{id} primer={match_sequence}' \
      --quiet \
      --fasta \
      "$@"
}

#### 1. forward searches
cutadapt_trim_primer -g $primer $demux > $out/cutadapt_trim_primer.fa
st_find_primer $primer $demux | $st trim -e '{attr(pend)}:' > $out/st_trim_primer.fa

# the primer alignments are slightly different in 16 of 13953 cases
$st cmp -k 'id' -d 'attr(primer)' $out/st_trim_primer.fa $out/cutadapt_trim_primer.fa
# ...but the primer-trimmed files are identical, as the primer end is always consistent
$st cmp -ck 'id,seq' $out/st_trim_primer.fa $out/cutadapt_trim_primer.fa

#### 2. search reverse complemented primer in reverse complemented reads
# (so the primer is now at the end)
cutadapt_trim_primer -a $primer_rc $demux_rev | st revcomp > $out/cutadapt_trim_primer_rev.fa
st_find_primer $primer_rc $demux_rev | $st trim -e ':{attr(pstart)}' | st revcomp > $out/st_trim_primer_rev.fa

# Forward and reverse searching differs for 224 of 13953 sequences both seqtool and cutadapt
$st cmp -k 'id,seq' $out/cutadapt_trim_primer.fa $out/cutadapt_trim_primer_rev.fa
$st cmp -k 'id,seq' $out/st_trim_primer.fa $out/st_trim_primer_rev.fa
# the reason: there can exist multiple hits with the same edit distance,
# then the left most hit is chosen; we can see that right-trimmed reads are shorter
# Cutadapt also picks the leftmost hit even with the -a option
$st cmp -k id -d seq $out/st_trim_primer.fa $out/st_trim_primer_rev.fa

# comparing cutadapt with seqtool: 9 of 13953 cases are different,
# in this case trimmed reads are also different since the match start is not always the same
$st cmp -k 'id' -d 'attr(primer)' $out/st_trim_primer_rev.fa $out/cutadapt_trim_primer_rev.fa
$st cmp -k 'id' -d 'seq' $out/st_trim_primer_rev.fa $out/cutadapt_trim_primer_rev.fa

# However, seqtool can also choose the last hit instead, trimming at match_start(-1) instead of match_start
# (cutadapt does not support finding last occurrence except if anchored)
st_find_primer $primer_rc $demux_rev | $st trim -e ':{attr(last_pstart)}' | st revcomp > $out/st_trim_primer_rev_last.fa
# Now, only 4 alignments are still slightly different due to the way the approximate searching works,
# but the difference in sequence length is small (3-5bp)
st cmp -k id -d seq $out/st_trim_primer.fa $out/st_trim_primer_rev_last.fa

#### 3. anchored searches
# Anchoring disallows hits that don't start at the beginning/end of the read;
# results are not always identical between seqtool and Cutadapt, and between forward and reverse searches

cutadapt_trim_primer -g ^$primer $demux > $out/cutadapt_trim_primer_anchor.fa
st_find_primer --anchor-start 0 $primer $demux | $st trim -e '{attr(pend)}:' > $out/st_trim_primer_anchor.fa
# Cutadapt finds 24 more alignments than seqtool (13922 trimmed reads are identical)
st cmp -k id -d seq $out/cutadapt_trim_primer_anchor.fa $out/st_trim_primer_anchor.fa --u1 $out/cutadapt_trim_primer_anchor_unique.fa
# Anchoring in seqtool works as expected
st_find_primer $primer $demux | $st filter 'attr("pstart") == 1' | $st trim -e '{attr(pend)}:' > $out/st_trim_primer_anchor2.fa
st cmp -k id -d seq $out/st_trim_primer_anchor.fa $out/st_trim_primer_anchor2.fa  # identical
# We can see that Cutadapt allows 1-3 extra bases on the left side, so the hit becomes anchored.
# Seqtool however does not adjust the alignment if anchoring is activated, so if the optimal
# alignment is not anchored, then the hit is discarded.
st cmp -k id -d 'attr(primer)' $out/cutadapt_trim_primer.fa $out/cutadapt_trim_primer_anchor_unique.fa
# We may allow a small overhang (then Cutadapt finds 1 additional, Seqtool finds 7 additional hits)
st_find_primer --anchor-start 1 $primer $demux | $st trim -e '{attr(pend)}:' > $out/st_trim_primer_anchor_overhang.fa
st cmp -k id -d seq $out/cutadapt_trim_primer_anchor.fa $out/st_trim_primer_anchor_overhang.fa

# anchored (reverse complement)
cutadapt_trim_primer -a "$primer_rc"'$' $demux_rev | st revcomp > $out/cutadapt_trim_primer_rev_anchor.fa
st_find_primer --anchor-end 0 $primer_rc $demux_rev | $st trim -e ':{attr(pstart)}' | st revcomp > $out/st_trim_primer_rev_anchor.fa
# Anchoring in seqtool works as expected
st_find_primer $primer_rc $demux_rev | $st filter 'attr("last_pend") == seqlen' | $st trim -e ':{attr(last_pstart)}' | st revcomp > $out/st_trim_primer_rev_anchor2.fa
st cmp -k id -d seq $out/st_trim_primer_rev_anchor.fa $out/st_trim_primer_rev_anchor2.fa # identical
# Same sequences forward/reverse for Cutadapt, 4 of 13950 are trimmed differently
st cmp -k id -d seq $out/cutadapt_trim_primer_anchor.fa $out/cutadapt_trim_primer_rev_anchor.fa
# Compared to reverse search: Cutadapt still finds 14 more (and 8 are trimmed differently)
st cmp -k id -d seq $out/cutadapt_trim_primer_rev_anchor.fa $out/st_trim_primer_rev_anchor.fa
# Seqtool: 3 of 13922 are unique to the forward search, 13 of 13932 are unique to the reverse search
st cmp -k id -d seq $out/st_trim_primer_anchor.fa $out/st_trim_primer_rev_anchor.fa

# overall counts
st count -k filename  $out/*_trim_primer*.fa
