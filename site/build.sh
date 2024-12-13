#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
    echo "This script must be run from the root of the repository"
    exit 1
fi

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
