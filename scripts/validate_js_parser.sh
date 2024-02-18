#!/usr/bin/env bash
# This script validates the JS parser using ECMA-262 parser tests
# All syntax should be accepted, exept for:
# - regex literals
# - \u unicode escapes in identifiers
# - characters that are not char::is_alphabetic() (there seem to be some)

set -euo pipefail

if [ ! -e test262-parser-tests-master ]; then
    wget https://github.com/tc39/test262-parser-tests/archive/refs/heads/master.zip
    unzip master.zip
    rm master.zip
fi

cargo build
st=target/debug/st

echo "" > _input.fa
js=test262-parser-tests-master
for f in $js/pass/*.js $js/pass-explicit/*.js $js/early/*.js; do
    echo $f
    out=$(($st . --to-tsv "{{file:$f}}" _input.fa || true) 2>&1)
    # recognize errors, but exclude strings containing unsupported character escaped
    if [[ "$out" == *"Failed to parse"* && ! "$out" =~ (\\0|\\u[0-9]{4}|\\u\{[a-zA-Z0-9]{1,6}\}|\\x[a-zA-Z0-9]{2}) ]]; then
        printf "$out"
    fi
done
rm _input.fa
