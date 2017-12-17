#!/bin/sh

# FASTQ file
f=$1
# primer seqs. for searching
seq1=$2
seq2=$3



alias s=target/release/seqtool

# prepare
# s . -p gc={s:gc} $f > $f.with_gc.fq
# gzip -k $f
# lz4 -k $f

# get files into memory cache
wc -l $f $f.* > /dev/null

logfile=timing.txt
exec > $logfile 2>&1
set -x

# conversion
time s . --to-fa $f > /dev/null
time read_fastq -i $f -e base_33 | write_fasta -x > /dev/null
time cat $f | fastq_to_fasta -Q33 > /dev/null
time fastq_to_fasta -Q33 -i $f > /dev/null
time seqtk seq -A $f > /dev/null
time seqkit fq2fa $f > /dev/null

# random subsampling
time s sample -f 0.1 $f > /dev/null
time seqtk sample $f 0.1 > /dev/null
time seqkit sample -p 0.1 $f > /dev/null

# counting
time s count $f
time read_fastq -i $f -e base_33 | count_records -x
time wc -l $f

# reverse complement (note, qualities are only reversed by seqtool)
time fastx_reverse_complement -i $f -Q33 > /dev/null
time read_fastq -i $f -e base_33 | reverse_seq | complement_seq | write_fastq -x > /dev/null
time s revcomp $f > /dev/null
time s revcomp -t4 $f > /dev/null
time seqtk seq -r $f > /dev/null
time seqkit seq -rp $f > /dev/null

# compress
time s . $f > /dev/null
time s . $f --outformat fastq.lz4 > /dev/null
time s . $f | lz4 -c > /dev/null
time s . $f --outformat fastq.gz > /dev/null
time s . $f | gzip -c > /dev/null

# decompress
time s . $f.lz4 > /dev/null
time lz4 -dc $f.lz4 | s . --fq > /dev/null
time s . $f.gz > /dev/null
time gzip -dc $f.gz | s . --fq > /dev/null
time seqtk seq $f.gz > /dev/null
time gzip -dc $f.gz | seqtk seq $f.gz > /dev/null

# RNA -> DNA
time s replace T U $f > /dev/null
time s replace T U $f -t4 > /dev/null
time s find T --rep U $f  > /dev/null
time s find T --rep U $f -t4 > /dev/null
time seqkit seq --dna2rna $f > /dev/null
time read_fastq -i $f -e base_33 | transliterate_vals -k SEQ -s T -r U | write_fastq -x > /dev/null
time fasta_nucleotide_changer -i $f -Q33 -r > /dev/null

# GC content "histogram"
time s count -k n:10:{s:gc} $f

# from variable
time s count -k n:10:{p:gc} $f.with_gc.fq

# with expression
time s count -k n:10:{{s:gc+0}} $f

# primer finding

printf ">primer1\n$seq1\n>primer2\n$seq2\n" > _primer_file.fa
fp=_primer_file.fa

time s find file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 -t4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --in-order file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --in-order -t4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --rng ..25 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --rng ..25 -t4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 file:$fp $f > /dev/null
time s find -d4 -t4 file:$fp $f > /dev/null
time s find -d4 --algo ukkonen file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --algo ukkonen -t4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --algo myers file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null
time s find -d4 --algo myers -t4 file:$fp $f -p primer={f:name} -p start={f:start} -p end={f:end} -p dist={f:dist} > /dev/null

time cutadapt -g primer1=^$seq1 -g primer2=^$seq2 $f -e 0.23 -y ' primer={name}' | s count --fq

