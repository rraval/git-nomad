_list:
    @just --list

test:
    cargo test

fmt:
    cargo fmt

clippy:
    cargo clippy --all

shellcheck:
    shellcheck -o all scripts/*.sh site/*.sh

lint: fmt clippy shellcheck

coverage:
    cargo llvm-cov --html --open

site:
    site/build.sh

# Bump the version and git commit (does not publish a GitHub release yet)
release:
    scripts/release.sh

# Demonstrate all features as an asciinema screencast
record-demo:
    @mkdir -p target/
    cargo clean
    cargo build --release
    PATH="$(pwd)/target/release:$PATH"                          \
        asciinema rec                                           \
        --cols 120                                              \
        --rows 36                                               \
        --command scripts/demo.sh                               \
        --title "$($(pwd)/target/release/git-nomad --version)"  \
        --overwrite target/demo.cast

# Upload the recorded demo to asciinema.org
upload-demo:
    @scripts/asciinema-upload.sh target/demo.cast
