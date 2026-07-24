#!/bin/bash
set -Eeuo pipefail

apt-get update
apt-get install -y \
  build-essential \
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
