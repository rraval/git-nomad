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

demo/record.sh
demo_url=$(demo/upload.sh)
curl -sSL -o site/out/demo.svg "${demo_url}.svg"

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

sed -i \
    -e 's|:DEMO_URL|'"${demo_url}"'|g' \
    site/out/demo.html
