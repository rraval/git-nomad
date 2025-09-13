_list:
    @just --list

test:
    cargo test

fmt:
    cargo fmt --all

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

shellcheck:
    shellcheck -o all release.sh site/*.sh demo/*.sh

lint: fmt clippy shellcheck

coverage:
    cargo llvm-cov --html --open

site:
    DEMO_FAST=1 site/build.sh

record-demo:
    demo/record.sh

upload-demo:
    demo/upload.sh

# Bump the version and git commit
release_commit:
    ./release_commit.sh
