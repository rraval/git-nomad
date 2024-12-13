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
    ref=${GITHUB_REF_NAME?}
    sha=${GITHUB_SHA?}
else
    ref=$(git describe --tags --always --dirty)
    sha=$(git rev-parse HEAD)
fi

sed -i \
    -e 's/:REF/'"${ref}"'/g' \
    -e 's/:SHA/'"${sha}"'/g' \
    site/out/index.html
