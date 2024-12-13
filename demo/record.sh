#!/usr/bin/env bash
# This script should be invoked from the root of the repository.
set -euo pipefail

if [[ -n "${CI-}" ]]; then
    REF=${GITHUB_REF_NAME}
else
    REF=$(git describe --tags --always --dirty)
fi

mkdir -p demo/out
GIT_NOMAD_BUILD_VERSION=${REF} cargo build --release
PATH="$PWD/target/release:$PATH"    \
    asciinema rec                   \
    --cols 120                      \
    --rows 36                       \
    --command demo/demo.sh          \
    --title "git-nomad ${REF}"      \
    --overwrite demo/out/demo.cast
