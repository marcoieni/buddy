#!/usr/bin/env bash
set -Eeuo pipefail

credentials_dir="$HOME/.config/buddy"
credentials_file="$credentials_dir/$1"

mkdir -p "$credentials_dir"
chmod 700 "$credentials_dir"
umask 077
cat >"$credentials_file"

source_line="[ ! -r \"$credentials_file\" ] || . \"$credentials_file\""
grep -Fqx "$source_line" "$HOME/.profile" ||
    printf '\n%s\n' "$source_line" >>"$HOME/.profile"
