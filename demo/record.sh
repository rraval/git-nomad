#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
    echo "This script must be run from the root of the repository"
    exit 1
fi

if [[ -n "${CI-}" ]]; then
    ref=${GITHUB_REF_NAME?}
else
    ref=$(git describe --tags --always --dirty)
fi

mkdir -p demo/out
GIT_NOMAD_BUILD_VERSION=${ref} cargo build --release
PATH="${PWD}/target/release:${PATH}"    \
    asciinema rec                       \
    --cols 120                          \
    --rows 36                           \
    --command demo/demo.sh              \
    --title "git-nomad ${ref}"          \
    --overwrite demo/out/demo.cast
