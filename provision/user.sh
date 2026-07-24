#!/usr/bin/env bash
set -Eeuo pipefail

append_line_if_missing() {
  local line="$1"
  local file="$2"

  if ! grep -Fqx "$line" "$file"; then
    printf '\n%s\n' "$line" >>"$file"
  fi
}

# Install homebrew if not present
brew_bin="/home/linuxbrew/.linuxbrew/bin/brew"
if [[ ! -x "$brew_bin" ]]; then
  NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

# Make brew available in the current shell session
brew_shellenv="eval \"\$(\"$brew_bin\" shellenv)\""
eval "$brew_shellenv"

# Make brew available to other login shells
append_line_if_missing "$brew_shellenv" "$HOME/.profile"

brew install datadog-labs/pack/pup
brew install fastly/tap/fastly

# Install or update codex
curl -fsSL https://chatgpt.com/codex/install.sh | sh
