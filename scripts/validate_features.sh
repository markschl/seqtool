#!/bin/bash
# This script runs the compilation and unit tests for each individual feature

set -euo pipefail

features=( \
    pass,gz \
    pass,lz4 \
    pass,zstd \
    pass,bz2 \
    expr \
    all-commands \
    all-commands,expr \
    pass \
    pass,regex-fast \
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
    find,regex-fast \
    replace \
    replace,regex-fast \
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

echo "===== NO features ======================"
echo -n "build... "
cargo build -q -j $cores --no-default-features
echo "test..."
cargo test -q -j $cores --no-default-features

echo "===== Default features ======================"
echo -n "build... "
cargo build -q -j $cores
echo "test..."
cargo test -q -j $cores

# single feature
for feature in ${features[@]}; do
    echo "===== Feature(s) '$feature' ======================"
    echo -n "build... "
    cargo build -q -j $cores --no-default-features --features=$feature
    echo "test..."
    cargo test -q -j $cores --no-default-features --features=$feature
done
