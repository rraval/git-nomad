#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
    echo "This script must be run from the root of the repository"
    exit 1
fi

# use `|| true` to prevent premature exit since grep returns non-zero if no
# matches are found.
num_tags=$(git tag --contains HEAD | grep --count '^v') || true
if [[ "${num_tags}" -gt 0 ]]; then
    echo "Found existing version tag on HEAD... nothing to do"
    exit 0
fi

# Extract the next intended version from the "Unreleased" section of the
# changelog.
next_version=$(sed -n -e '/^##.*Unreleased/ s/^.*v\([0-9]\+\.[0-9]\+\.[0-9]\+\).*$/\1/p' CHANGELOG.md)

if [[ -z "${next_version}" ]]; then
    echo "Next version not found in CHANGELOG.md"
    exit 1
fi

# In-place edit of various files
today=$(date +%Y-%m-%d)
# Assume version is always on line 3
sed -i -e '3c version = "'"${next_version}"'"' Cargo.toml
sed -i -e '/^##.*Unreleased/{
c\
## [vX.X.X] - Unreleased\
\
## ['"${next_version}"'] - '"${today}"'
}' CHANGELOG.md

# Sanity check and regenerates `Cargo.lock` with updated version from `Cargo.toml`
cargo check

# Get it into git
git add Cargo.toml Cargo.lock CHANGELOG.md README.md
git commit -m "Release v${next_version}"
git tag v"${next_version}"
