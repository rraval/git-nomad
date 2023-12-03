_list:
    @just --list

# Bump the version and git commit (does not publish a GitHub release yet)
release:
    scripts/release.sh

# Demonstrate all features as an asciinema screencast
record-demo:
    @mkdir -p target/
    cargo build --release
    PATH="$(pwd)/target/release:$PATH"                          \
        asciinema rec                                           \
        --cols 120                                              \
        --rows 36                                               \
        --command scripts/demo.sh                               \
        --title "$($(pwd)/target/release/git-nomad --version)"  \
        --overwrite target/demo.cast

# Upload the recorded demo to asciinema.org
upload-demo: record-demo
    #!/usr/bin/env bash
    set -euo pipefail

    if [[ -z "${ASCIINEMA_INSTALL_ID:-}" ]]; then
        echo 'Must specify $ASCIINEMA_INSTALL_ID'
        exit 1
    fi

    asciinema_config=$(mktemp -d)
    trap "rm -rf '${asciinema_config}'" EXIT

    echo "${ASCIINEMA_INSTALL_ID}" > "${asciinema_config}"/install-id
    export ASCIINEMA_CONFIG_HOME="${asciinema_config}"

    asciinema upload target/demo.cast
