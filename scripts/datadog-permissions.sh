#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
    echo "Usage: datadog-permissions.sh {dump|assert} VM SNAPSHOT" >&2
    exit 2
}

action="${1:-}"
vm="${2:-}"
snapshot="${3:-}"

if [[ "$action" != dump && "$action" != assert ]] || [[ -z "$vm" || -z "$snapshot" ]]; then
    usage
fi

secret_reference="op://Infrastructure/datadog-read-only/credential"
temporary_snapshot=""

cleanup() {
    if [[ -n "$temporary_snapshot" ]]; then
        rm -f -- "$temporary_snapshot"
    fi
}
trap cleanup EXIT

assert_current_token() {
    local expected_token
    local expected_digest
    local guest_digest

    expected_token="$(op read "$secret_reference")"
    if [[ "$expected_token" != ddsat_* ]]; then
        echo "Expected the 1Password item to contain a Datadog service access token (prefix: ddsat_)." >&2
        return 1
    fi

    expected_digest="$(printf '%s' "$expected_token" | shasum -a 256 | awk '{print $1}')"
    unset expected_token

    # Hash the guest token inside the VM. Only the digest crosses the VM
    # boundary, and neither token is exposed in a command argument.
    guest_digest="$(
        # Variables in this script expand in the guest.
        # shellcheck disable=SC2016
        limactl shell "$vm" bash -lc '
            set -Eeuo pipefail
            if [[ "${DD_ACCESS_TOKEN:-}" != ddsat_* ]]; then
                echo "The guest does not have a Datadog service access token. Run: just login-datadog" >&2
                exit 1
            fi
            printf "%s" "$DD_ACCESS_TOKEN" | sha256sum | awk "{print \$1}"
        '
    )"

    if [[ "$guest_digest" != "$expected_digest" ]]; then
        echo "The guest is not using the current Datadog token from 1Password. Run: just login-datadog" >&2
        return 1
    fi
}

write_live_snapshot() {
    # `pup` has no dedicated permissions command. Its raw API command can call
    # current_user, whose response includes the authenticated service account,
    # its roles, and every permission granted by those roles. The access-token
    # endpoint adds the scopes granted to this specific token.
    {
        limactl shell "$vm" bash -lc '
            set -Eeuo pipefail
            pup api v2/current_user --output json --read-only
        '

        # Keep other access-token metadata inside the guest. Only the scopes of
        # the token already present in DD_ACCESS_TOKEN cross the VM boundary.
        # shellcheck disable=SC2016
        limactl shell "$vm" bash -lc '
            set -Eeuo pipefail
            pup api v2/personal_access_tokens \
                --field "filter=buddy" \
                --field "page[size]=100" \
                --output json \
                --read-only |
                jq -e "
                    [
                        .data[]
                        | select(.type == \"service_access_tokens\")
                        | . as \$access_token
                        | select(
                            env.DD_ACCESS_TOKEN
                            | startswith(\$access_token.attributes.public_portion)
                        )
                    ]
                    | if length == 1 then
                        {scopes: (.[0].attributes.scopes | sort)}
                      else
                        error(
                            \"expected exactly one access-token record matching DD_ACCESS_TOKEN\"
                        )
                      end
                "
        '
    } |
        jq -e --slurp '
            if length != 2 then
                error("expected current-user and access-token responses")
            else
                .
            end
            | .[0] as $response
            | .[1] as $token
            | {
                schema_version: 1,
                service_account: {
                    id: $response.data.id,
                    name: $response.data.attributes.name
                },
                token_scopes: $token.scopes,
                roles: (
                    [
                        $response.included[]?
                        | select(.type == "roles")
                        | {
                            id,
                            name: .attributes.name,
                            receives_permissions_from: .attributes.receives_permissions_from,
                            permissions: (
                                [.relationships.permissions.data[]?.id]
                                | sort
                            )
                        }
                    ]
                    | sort_by(.id)
                ),
                permissions: (
                    [
                        $response.included[]?
                        | select(.type == "permissions")
                        | {
                            id,
                            name: .attributes.name,
                            display_name: .attributes.display_name,
                            description: .attributes.description,
                            group_name: .attributes.group_name,
                            name_aliases: (.attributes.name_aliases // [] | sort),
                            restricted: .attributes.restricted
                        }
                    ]
                    | sort_by(.id)
                )
            }
            | if .service_account.id == null or .service_account.name == null then
                error("current_user did not return a service-account identity")
              elif (.token_scopes | length) == 0 then
                error("the service access token did not return any scopes")
              elif (.roles | length) == 0 then
                error("current_user did not return any roles")
              elif (.permissions | length) == 0 then
                error("current_user did not return any permissions")
              else
                .
              end
        '
}

assert_current_token

case "$action" in
    dump)
        mkdir -p -- "$(dirname -- "$snapshot")"
        temporary_snapshot="$(mktemp "${snapshot}.tmp.XXXXXX")"
        write_live_snapshot >"$temporary_snapshot"
        chmod 0644 "$temporary_snapshot"
        mv -- "$temporary_snapshot" "$snapshot"
        temporary_snapshot=""
        echo "Wrote the current Datadog permissions to $snapshot"
        ;;
    assert)
        if [[ ! -r "$snapshot" ]]; then
            echo "Datadog permissions snapshot not found: $snapshot" >&2
            echo "Create it with: just dump-datadog-permissions" >&2
            exit 1
        fi

        temporary_snapshot="$(mktemp "${TMPDIR:-/tmp}/buddy-datadog-permissions.XXXXXX")"
        write_live_snapshot >"$temporary_snapshot"

        if ! diff -u -- "$snapshot" "$temporary_snapshot"; then
            echo "The Datadog identity, roles, or permissions do not match the documented snapshot." >&2
            echo "If the change is intentional, review it with: just dump-datadog-permissions" >&2
            exit 1
        fi

        echo "Datadog credentials are current and their permissions match $snapshot"
        ;;
esac
