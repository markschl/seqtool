# Measuring time and memory / comparison to other tools

The following should work for Ubuntu Linux.

```bash
outdir=target/st_benchmark
fq=$outdir/reads.fq
mkdir -p $outdir
```

## Build the binary

```bash
cargo build --release
st=target/release/st
```

## Download sequencing reads


```bash
wget -qi profile/fastq_urls.txt -O - | zcat > $fq
ls -lh $fq
```

## Create temporary storage

We rely on *tmpfs* to store output (and some input) files in memory,
avoiding disk IO latency as much as possible.

```bash
rm -Rf $outdir/workdir && mkdir $outdir/workdir
chmod 777 $outdir/workdir
sudo mount -t tmpfs -o size=10G none $outdir/workdir
mkdir -p $outdir/workdir/tmp
```

Prepare forward primer for searching

```bash
cat > $outdir/workdir/primers.fasta <<- EOM
>ITS4
GTCCTCCGCTTATTGATATGC
EOM
```

## Run the benchmarks

Before running, disable frequency boost:

```bash
echo "0" | sudo tee /sys/devices/system/cpu/cpufreq/boost
```

On Ubuntu, disable the indexer for full-text search

```bash
echo -n > $outdir/workdir/.trackerignore
```

Run the comparison. The `compare_tools.py` does not only compare runtimes / memory usage,
but in some cases also validates that the output is the same.
See `comparison_commands.yaml`.

```bash
export SEQKIT_THREADS=1
$st count $fq  # cache the file in memory
scripts/compare_tools.py \
    -b $st -d $outdir/workdir -o profile/comparison.json -t $outdir/workdir/tmp \
    $fq profile/comparison_commands.yaml 

scripts/summarize_comparison.py profile/comparison.json - > profile/comparison.md
```

## Clean up

```bash
rm -Rf $outdir/workdir
```
