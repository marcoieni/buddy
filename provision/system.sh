#!/bin/bash
set -Eeuo pipefail

apt-get update

# Codex uses bubblewrap for its Linux sandbox. This is needed if you want to run codex withot full access.
apt-get install -y \
  build-essential \
  bubblewrap \
  ca-certificates \
  curl \
  file \
  git \
  htop \
  jq \
  pkg-config \
  procps \
  python3 \
  python3-pip \
  python3-venv \
  ripgrep \
  rsync \
  unzip \
  zip

# Since we are using snap, the aws-cli will be automatically updated.
snap install aws-cli --classic
