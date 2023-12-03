#!/usr/bin/env bash
set -euo pipefail

# Assume `version` is always on line 3
base_version_str=$(sed -n -e '3{s/^version = "//; s/"$//; p}' Cargo.toml)

IFS='.' read -r -a version_array <<< "$base_version_str"
if [[ "${#version_array[@]}" -ne 3 ]]; then
    echo "$base_version_str does not match the expected format"
    exit 1
fi

# Always increment patch version
version_array[2]=$((version_array[2] + 1))

new_version_str="${version_array[0]}.${version_array[1]}.${version_array[2]}"

# In-place edit of various files
sed -i -e '3c version = "'"$new_version_str"'"' Cargo.toml
sed -i -e '/^## \[Unreleased\]$/{
a\

a\
## ['"$new_version_str"'] - '"$(date +%Y-%m-%d)"'

}' CHANGELOG.md

# Sanity check and regenerates `Cargo.lock` with updated version from `Cargo.toml`
cargo check

# Get it into git
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release $new_version_str"
