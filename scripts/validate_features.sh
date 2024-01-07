#!/bin/bash

# This scripts runs the compilation and unit tests for each individual feature

set -euo pipefail

features=( \
    "" \
    gz,pass lz4,pass zstd,pass bz2,pass \
    expr \
    all_commands,expr \
    pass \
    view \
    count \
    stat \
    head \
    tail \
    slice \
    sample \
    sort \
    unique \
    filter,expr \
    split \
    interleave \
    find \
    replace \
    del \
    set \
    trim \
    mask \
    upper \
    lower \
    revcomp \
    concat \
)

cores=8

# no features at all
echo "===== NO features ======================"
echo -n "build... "
RUSTFLAGS=-Awarnings cargo build -q -j $cores --no-default-features
echo "test..."
RUSTFLAGS=-Awarnings cargo test -q -j $cores --no-default-features

# single feature
for feature in ${features[@]}; do
    echo "===== Feature '$feature' ======================"
    echo -n "build... "
    RUSTFLAGS=-Awarnings cargo build -q -j $cores --no-default-features --features=$feature
    echo "test..."
    RUSTFLAGS=-Awarnings cargo test -q -j $cores --no-default-features --features=$feature
done
