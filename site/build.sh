#!/usr/bin/env bash
# This script should be invoked from the root of the repository.
set -euo pipefail

rm -rf site/out
cp -r site/src site/out
cargo doc --all-features
cp -r target/doc site/out/doc

version=$(git describe --tags --always --dirty)
commit=$(git rev-parse HEAD)

sed -i \
    -e 's/:VERSION/'"${version}"'/g' \
    -e 's/:COMMIT/'"${commit}"'/g' \
    site/out/index.html
