#!/usr/bin/env bash
set -Eeuo pipefail

# shellcheck source=scripts/login-common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/login-common.sh"

vm="${1:?Usage: login-fastly.sh VM}"

# Fetch the automation token on the host.
api_token="$(op read "op://Infrastructure/fastly-read-only/credential")"

# Fastly does not document a stable prefix for API tokens, so authentication
# below is the authoritative validation.
if [[ -z "$api_token" ]]; then
    echo "Expected a non-empty Fastly API token." >&2
    exit 1
fi

# Send shell-escaped assignments over stdin instead of exposing the token in
# the guest command's arguments.
printf 'export FASTLY_API_TOKEN=%q\nexport FASTLY_DISABLE_AUTH_COMMAND=1\n' "$api_token" |
    install_guest_credentials "$vm" fastly.env

# Verify the saved credentials with a minimal read-only API request. The
# externally managed environment intentionally disables `fastly whoami`.
limactl shell "$vm" bash -lc 'fastly service list --per-page 1 >/dev/null'
echo "Fastly authentication is configured and working."
