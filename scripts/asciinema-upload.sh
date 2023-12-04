#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
    echo "Usage: $(basename "$0") PATH_TO_CAST_FILE"
    exit 1
fi

cast_file="$1"

if [[ -z "${ASCIINEMA_INSTALL_ID:-}" ]]; then
    echo 'Must specify ASCIINEMA_INSTALL_ID'
    exit 1
fi

asciinema_config=$(mktemp -d)
# we're okay with this expanding immediately
# shellcheck disable=SC2064
trap "rm -rf '${asciinema_config}'" EXIT

echo "${ASCIINEMA_INSTALL_ID}" > "${asciinema_config}"/install-id
export ASCIINEMA_CONFIG_HOME="${asciinema_config}"

asciinema upload "${cast_file}" 2>&1 | sed -n -e '/asciinema.org/{ s/^ \+//; p }'
