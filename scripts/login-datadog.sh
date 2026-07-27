#!/usr/bin/env bash
set -Eeuo pipefail

# shellcheck source=scripts/login-common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/login-common.sh"

vm="${1:?Usage: login-datadog.sh VM}"

# Fetch the service token on the host.
access_token="$(op read "op://Infrastructure/datadog-read-only/credential")"
site="https://app.datadoghq.com/"

# Fail early if the 1Password item contains the wrong kind of credential.
if [[ "$access_token" != ddsat_* ]]; then
    echo "Expected a Datadog service access token (prefix: ddsat_)." >&2
    exit 1
fi

# Send shell-escaped assignments over stdin instead of exposing the token in
# the guest command's arguments.
printf 'export DD_ACCESS_TOKEN=%q\nexport DD_SITE=%q\n' "$access_token" "$site" |
    install_guest_credentials "$vm" datadog.env

# Verify the saved credentials with a minimal read-only API request.
limactl shell "$vm" bash -lc 'pup monitors list --limit 1 --read-only >/dev/null'
echo "Datadog authentication is configured and working."
