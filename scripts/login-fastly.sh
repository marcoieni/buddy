#!/usr/bin/env bash
set -Eeuo pipefail

vm="${1:?Usage: login-fastly.sh VM}"

# Fetch the automation token on the host.
api_token="$(op read "op://Infrastructure/fastly-read-only/credential")"

# Fastly does not document a stable prefix for API tokens, so authentication
# below is the authoritative validation.
if [[ -z "$api_token" ]]; then
    echo "Expected a non-empty Fastly API token." >&2
    exit 1
fi

# Send a shell-escaped assignment over stdin instead of exposing the token in
# the guest command's arguments.
# The variables in the single-quoted script expand in the guest.
# shellcheck disable=SC2016
printf 'export FASTLY_API_TOKEN=%q\nexport FASTLY_DISABLE_AUTH_COMMAND=1\n' "$api_token" |
    limactl shell "$vm" bash -c '
        set -Eeuo pipefail
        credentials_dir="$HOME/.config/buddy"
        credentials_file="$credentials_dir/fastly.env"

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

# Verify the saved credentials with a minimal read-only API request. The
# externally managed environment intentionally disables `fastly whoami`.
limactl shell "$vm" bash -lc 'fastly service list --per-page 1 >/dev/null'
echo "Fastly authentication is configured and working."
