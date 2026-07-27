#!/usr/bin/env bash
set -Eeuo pipefail

pup api v2/personal_access_tokens \
    --field "filter=buddy" \
    --field "page[size]=100" \
    --output json \
    --read-only |
    jq --exit-status '
        [
            .data[]
            | select(.type == "service_access_tokens")
            | . as $access_token
            | select(
                env.DD_ACCESS_TOKEN
                | startswith($access_token.attributes.public_portion)
            )
        ]
        | if length == 1 then
            {scopes: (.[0].attributes.scopes | sort)}
          else
            error(
                "expected exactly one access-token record matching DD_ACCESS_TOKEN"
            )
          end
    '
