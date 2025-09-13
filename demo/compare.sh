#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "Usage: $0 <file1> <file2>"
    exit 1
fi

git_diff_output=$(git diff --no-index --numstat "$1" "$2")
IFS=$'\t' read -r -a stat <<< "${git_diff_output}"

if [[ -z "${stat[0]-}" ]]; then
    echo "Demo unchanged"
else
    echo "<details><summary>Demo changed, ${stat[0]} insertions(+), ${stat[1]} deletions(-)</summary>"
    echo
    echo '```diff'
    git diff --no-index --no-prefix "$1" "$2" || true
    echo '```'
    echo "</details>"
fi
