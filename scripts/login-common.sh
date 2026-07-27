#!/usr/bin/env bash

install_guest_credentials() {
    local vm="${1:?Expected a VM name.}"
    local credentials_name="${2:?Expected a credentials filename.}"

    if [[ "$credentials_name" == */* ]]; then
        echo "Expected a credentials filename, not a path." >&2
        return 1
    fi

    # Credential assignments arrive over stdin so they are not exposed in the
    # guest command's arguments. The variables in this script expand in the
    # guest.
    # shellcheck disable=SC2016
    limactl shell "$vm" bash -c '
        set -Eeuo pipefail
        credentials_dir="$HOME/.config/buddy"
        credentials_file="$credentials_dir/$1"

        # Restrict both the credentials directory and newly created files.
        mkdir -p "$credentials_dir"
        chmod 700 "$credentials_dir"
        umask 077
        cat >"$credentials_file"

        # Make future login shells load the credentials exactly once.
        source_line="[ ! -r \"$credentials_file\" ] || . \"$credentials_file\""
        grep -Fqx "$source_line" "$HOME/.profile" ||
            printf "\n%s\n" "$source_line" >>"$HOME/.profile"
    ' bash "$credentials_name"
}
