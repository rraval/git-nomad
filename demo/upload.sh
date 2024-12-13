#!/usr/bin/env bash
# This script should be invoked from the root of the repository.
set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
    echo "This script must be run from the root of the repository"
    exit 1
fi

if [[ ! -f demo/out/demo.cast ]]; then
    echo "No demo to upload"
    exit 1
fi

if [[ -n "${ASCIINEMA_INSTALL_ID:-}" ]]; then
    if [[ -n "${CI-}" ]]; then
        echo ":add-mask::${ASCIINEMA_INSTALL_ID}"
    fi
    mkdir -p "${HOME}/.config/asciinema/"
    echo "${ASCIINEMA_INSTALL_ID}" > "${HOME}/.config/asciinema/install-id"
fi

output=$(asciinema upload demo/out/demo.cast 2>&1)
url=$(sed -n -e '/asciinema.org\/a\//{ s/^ \+//; p }' <<< "${output}")
echo "${url}"
