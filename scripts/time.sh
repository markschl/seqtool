#!/bin/sh

# FASTQ file
f=$1
# primer seqs. for searching
seq1=$2
seq2=$3


alias s=target/release/seqtool

# prepare
# s . -a gc={s:gc} $f > $f.with_gc.fq
# s . --qual-out $f.qual --to-fa $f > /dev/null
# s . --to-fa $f > $f.fa
# gzip -k $f
# lz4 -k $f
# bzip2 -k $f
# zstd -k $f

# load files into memory
s count $f $f.* -k filename

logfile=timing.txt
exec > $logfile 2>&1
set -x

# conversion
time s . --to-fa $f > /dev/null
time s . --to fastq-illumina $f > /dev/null
time s . --qual $f.qual $f.fa > /dev/null
time s . --to-fq --qual $f.qual $f.fa > /dev/null
time read_fastq -i $f -e base_33 | write_fasta -x > /dev/null
time cat $f | fastq_to_fasta -Q33 > /dev/null
time fastq_to_fasta -Q33 -i $f > /dev/null
time seqtk seq -A $f > /dev/null
time seqkit fq2fa $f > /dev/null
time seqkit convert --from 'Sanger' --to 'Illumina-1.3+' $f > /dev/null

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
time s . $f --to fastq.lz4 > /dev/null
time s . $f | lz4 -c > /dev/null
time s . $f --to fastq.gz > /dev/null
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
time s count -k n:10:{a:gc} $f.with_gc.fq

# with expression
time s count -k n:.1:{{s:gc/100}} $f

# filter by length
time s filter 's:seqlen >= 100' $f > /dev/null
time seqtk seq -L 100 $f > /dev/null
time seqkit seq -m 100 $f > /dev/null
time read_fasta -i $f | grab -e 'SEQ_LEN >= 100' | write_fasta -x > /dev/null

# filter by quality
time s filter 's:exp_err < 1' $f --to-fa > /dev/null
time usearch -fastq_filter $f -fastq_maxee 1 -fastaout $f.filter.fa
time vsearch -fastq_filter $f -fastq_maxee 1 -fastaout $f.filter.fa
rm $f.filter.fa

# primer finding

printf ">primer1\n$seq1\n>primer2\n$seq2\n" > _primer_file.fa
fp=_primer_file.fa
printf "$seq1\n$seq2\n" | tr 'YR' 'N' > _primer_list.txt
sed 's/R/[AG]/g' _primer_file.fa > _primer_file_ambig.fa

run_find() {
    time s find -v file:$1 $f -a primer={f:name} -a rng={f:range} "${@:2}" > /dev/null
    time s find -v file:$1 $f -a primer={f:name} -a rng={f:range} -t4 "${@:2}" > /dev/null
}

run_find $fp --algo myers
run_find $fp --algo myers -d1
run_find $fp --algo myers -d4
run_find $fp --algo myers -d8
run_find $fp --algo myers -d4 --in-order
run_find $fp --algo myers -d4 --rng ..25
time s find -v file:$fp $f -a d={f:dist} > /dev/null
time s find -v file:$fp $f -a d={f:dist} -t4 > /dev/null
run_find $fp --algo exact
run_find _primer_file_ambig.fa -r --seqtype other

adapter_removal() {
    time AdapterRemoval --file1 $f --adapter-list _primer_list.txt --shift 8 --threads 4 \
     --output1 /dev/null --discarded /dev/stdout --settings /dev/null "$@" > /dev/null
    time AdapterRemoval --file1 $f --adapter-list _primer_list.txt --shift 8 --threads 4 \
    --output1 /dev/null --discarded /dev/stdout --settings /dev/null --threads 4 "$@" > /dev/null
}

adapter_removal --mm 1
adapter_removal --mm 4
adapter_removal --mm 8

time cutadapt -a primer1=$seq1$ -a primer2=$seq2$ $f -e 0.23 -y ' primer={name}' > /dev/null
time cutadapt -a primer1=$seq1$ -a primer2=$seq2$ $f -e 0.23 -y ' primer={name}' -j4 > /dev/null

# trim

time s find -f file:$fp $f -a primer={f:name} -a end={f:end} -t5 > $f.find.fq
time s trim -e {a:end}.. $f.find.fq > /dev/null
