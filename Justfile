_list:
    @just --list

test:
    cargo test

fmt:
    cargo fmt

clippy:
    cargo clippy --all

shellcheck:
    shellcheck -o all scripts/*.sh site/*.sh demo/*.sh

lint: fmt clippy shellcheck

coverage:
    cargo llvm-cov --html --open

site:
    site/build.sh

record-demo:
    demo/record.sh

upload-demo:
    demo/upload.sh

# Bump the version and git commit (does not publish a GitHub release yet)
release:
    scripts/release.sh
