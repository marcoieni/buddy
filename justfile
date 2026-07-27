# Docs: https://just.systems/man/en/quick-start.html

# `just` displays all available recipes when run without arguments.
# If false, `just` runs the first recipe when run without arguments.
set default-list := true

vm := "buddy"

host:
    mkdir -p "{{ env('HOME') / 'buddy' }}"

create: host
    limactl start --name "{{ vm }}" buddy.yaml

start:
    limactl start "{{ vm }}"

# "-" tells just to ignore command failure
stop:
    -limactl stop "{{ vm }}"

delete: stop
    limactl delete --force "{{ vm }}"

login: login-codex login-datadog login-fastly

login-codex:
    limactl shell "{{ vm }}" bash -lc 'codex login --device-auth'

login-datadog:
    cargo run --quiet -- login-datadog "{{ vm }}"

dump-datadog-permissions:
    cargo run --quiet -- datadog-permissions dump "{{ vm }}" docs/datadog-permissions.json

assert-datadog-credentials:
    cargo run --quiet -- datadog-permissions assert "{{ vm }}" docs/datadog-permissions.json

login-fastly:
    cargo run --quiet -- login-fastly "{{ vm }}"

# Upgrade manually. Upgrades are not done in `system.sh` because
# a full upgrade would make startup slower, less predictable, and could install kernel updates requiring another reboot.
upgrade:
    limactl shell "{{ vm }}" sudo apt-get upgrade
    limactl shell "{{ vm }}" bash -lc 'brew update && brew upgrade --yes'

rebuild: delete create

validate:
    limactl template validate buddy.yaml
    shellcheck provision/*.sh
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

shell:
    limactl shell "{{ vm }}"
