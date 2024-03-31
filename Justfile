_list:
    @just --list

# Bump the version and git commit (does not publish a GitHub release yet)
release:
    scripts/release.sh

# Run the demo as fast as possible
demo:
    cargo build --release
    PATH="$(pwd)/target/release:$PATH" scripts/demo.sh --fast

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
