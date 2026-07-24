#!/usr/bin/env bash
set -Eeuo pipefail

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
# The variables in the single-quoted script expand in the guest.
# shellcheck disable=SC2016
printf 'export DD_ACCESS_TOKEN=%q\nexport DD_SITE=%q\n' "$access_token" "$site" |
    limactl shell "$vm" bash -c '
        set -Eeuo pipefail
        credentials_dir="$HOME/.config/buddy"
        credentials_file="$credentials_dir/datadog.env"

        # Restrict both the credentials directory and newly created files.
        mkdir -p "$credentials_dir"
        chmod 700 "$credentials_dir"
        umask 077
        cat >"$credentials_file"

        # Make future login shells load the credentials exactly once.
        source_line="[ ! -r \"$credentials_file\" ] || . \"$credentials_file\""
        grep -Fqx "$source_line" "$HOME/.profile" ||
            printf "\n%s\n" "$source_line" >>"$HOME/.profile"
    '

# Verify the saved credentials with a minimal read-only API request.
limactl shell "$vm" bash -lc 'pup monitors list --limit 1 --read-only >/dev/null'
echo "Datadog authentication is configured and working."
