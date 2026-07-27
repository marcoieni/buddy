#!/usr/bin/env bash
set -Eeuo pipefail

pup api v2/current_user --output json --read-only
