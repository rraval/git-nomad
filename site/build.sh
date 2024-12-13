#!/usr/bin/env bash
# This script should be invoked from the root of the repository.
set -euo pipefail

rm -rf site/out
cp -r site/src site/out
cargo doc --all-features
cp -r target/doc site/out/doc

if [[ -n "${CI-}" ]]; then
    REF=${GITHUB_REF_NAME}
    SHA=${GITHUB_SHA}
else
    REF=$(git describe --tags --always --dirty)
    SHA=$(git rev-parse HEAD)
fi

sed -i \
    -e 's/:REF/'"${REF}"'/g' \
    -e 's/:SHA/'"${SHA}"'/g' \
    site/out/index.html
