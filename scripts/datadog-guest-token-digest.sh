#!/usr/bin/env bash
set -Eeuo pipefail

if [[ "${DD_ACCESS_TOKEN:-}" != ddsat_* ]]; then
    echo "The guest does not have a Datadog service access token. Run: just login-datadog" >&2
    exit 1
fi
printf '%s' "$DD_ACCESS_TOKEN" | sha256sum | awk '{print $1}'
