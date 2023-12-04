#!/usr/bin/env bash
set -euo pipefail

toplevel=$(git rev-parse --show-toplevel)
cd "${toplevel}"

# use `|| true` to prevent premature exit since grep returns non-zero if no
# matches are found.
num_tags=$(git tag --contains HEAD | grep --count '^v') || true
if [[ "${num_tags}" -gt 0 ]]; then
    echo "Found existing version tag on HEAD... nothing to do"
    exit 0
fi

# Assume `version` is always on line 3
base_version_str=$(sed -n -e '3{s/^version = "//; s/"$//; p}' Cargo.toml)

IFS='.' read -r -a version_array <<< "${base_version_str}"
if [[ "${#version_array[@]}" -ne 3 ]]; then
    echo "${base_version_str} does not match the expected format"
    exit 1
fi

# Always increment patch version
version_array[2]=$((version_array[2] + 1))

new_version_str="${version_array[0]}.${version_array[1]}.${version_array[2]}"

# In-place edit of various files
today=$(date +%Y-%m-%d)
sed -i -e '3c version = "'"${new_version_str}"'"' Cargo.toml
sed -i -e '/^## \[Unreleased\]$/{
a\

a\
## ['"${new_version_str}"'] - '"${today}"'

}' CHANGELOG.md

# Sanity check and regenerates `Cargo.lock` with updated version from `Cargo.toml`
cargo check

# Override the version so it appears in the title and output of the demo.
GIT_NOMAD_BUILD_VERSION="v${new_version_str}" just record-demo
asciinema_url=$(just upload-demo)
sed -i -e '/\[!\[asciicast\]/c '"[![asciicast](${asciinema_url}.svg)](${asciinema_url}?autoplay=1)" README.md

# Get it into git
git add Cargo.toml Cargo.lock CHANGELOG.md README.md
git commit -m "Release ${new_version_str}"
git tag v"${new_version_str}"
