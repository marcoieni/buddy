# Read-only observability reference

Use this reference for safe command shapes and for locating Rust
Infrastructure configuration. Check `--help` because installed CLI versions
and flags can change.

## Credential and permission boundary

Both CLIs and their tokens are installed in the Buddy guest environment.
Authentication is loaded automatically. Do not read credential files or print
environment-variable values.

Run every `pup` command with `--read-only`. The current Datadog `safe-readonly`
role grants:

- `logs_read_data`
- `logs_read_index_data`
- `logs_live_tail`
- `apm_read`
- `monitors_read`
- `dashboards_read`

Metrics, events, infrastructure hosts, incident management, synthetics, and
other products may require additional read-only permissions. An authorization
failure is a permission finding, not evidence that the data does not exist.

The Fastly automation token has the User role and exactly `global:read`, no TLS
management access, and an expiration date. It cannot read Secret Store values.
It can read configuration such as VCL, so treat generated VCL as sensitive and
inspect it only when necessary.

Never use Fastly `--debug-mode`; request and response diagnostics can expose
sensitive headers or content.

## Datadog command patterns

Start with small limits. Preserve explicit `--from` and `--to` values in the
investigation notes, preferably as RFC 3339 UTC timestamps.

List and inspect monitors:

```sh
pup monitors list \
  --name '<name fragment>' \
  --tags 'env:prod,service:<service>' \
  --limit 100 \
  --output json \
  --read-only

pup monitors get '<monitor-id>' --output json --read-only
```

Search logs:

```sh
pup logs search \
  --query 'env:prod service:<service> status:error' \
  --from '<start UTC>' \
  --to '<end UTC>' \
  --limit 100 \
  --sort asc \
  --output json \
  --read-only
```

Begin with tags from IaC and widen one dimension at a time. A missing log may
be in another index or storage tier; note this before concluding absence.

Inspect APM service statistics:

```sh
pup apm services list \
  --env prod \
  --from '<start UTC>' \
  --to '<end UTC>' \
  --output json \
  --read-only

pup apm services stats \
  --env prod \
  --from '<start UTC>' \
  --to '<end UTC>' \
  --output json \
  --read-only
```

List dashboards and retrieve only a relevant dashboard:

```sh
pup dashboards list --output json --read-only
pup dashboards get '<dashboard-id>' --output json --read-only
```

The CLI also exposes potentially useful read commands such as:

```sh
pup metrics query --query '<metric query>' \
  --from '<start UTC>' --to '<end UTC>' --output json --read-only

pup events search --query '<event query>' \
  --from '<start UTC>' --to '<end UTC>' --limit 100 \
  --output json --read-only

pup infrastructure hosts list \
  --filter '<filter>' --count 100 --output json --read-only
```

The current token may not authorize these last three product areas. If they
fail, capture the authorization error and request only the specific additional
read-only permission needed.

Use `--jq` to reduce output when the response schema is known. Never use a
mutating subcommand even with `--read-only`; the flag is a defense in depth,
not authorization to attempt writes.

## Fastly command patterns

Discover a service and record its ID:

```sh
fastly service list --json --per-page 100
fastly service describe --service-name '<name>' --json
```

Inspect deployed configuration:

```sh
fastly service version list --service-id '<service-id>' --json
fastly service domain list \
  --service-id '<service-id>' --version active --json
fastly service backend list \
  --service-id '<service-id>' --version active --json
```

Retrieve generated VCL only when required to test a configuration hypothesis:

```sh
fastly service vcl describe \
  --service-id '<service-id>' --version active --json
```

Stop if the VCL contains a credential or other secret. Do not paste it into the
report.

Inspect bounded historical CDN statistics:

```sh
fastly stats historical \
  --service-id '<service-id>' \
  --from '<start>' \
  --to '<end>' \
  --by minute \
  --json
```

Use `--field` for one documented statistic and `--region` only after checking
available region names. Compare requests, error status classes, origin
behavior, cache outcomes, and bandwidth as appropriate. Match the interval to
the Datadog query.

Do not use commands whose verbs imply a mutation, including create, update,
delete, activate, deploy, purge, enable, disable, or upload.

## `simpleinfra` source map

The repository evolves; verify paths in the inspected commit rather than
treating this list as exhaustive.

- `terragrunt/accounts/`: deployment entrypoints grouped by account,
  environment, and component. Trace each `terragrunt.hcl` to the referenced
  module and inputs.
- `terragrunt/modules/`: reusable infrastructure modules. Current examples
  include crates.io, release distribution, AWS-to-Datadog integration, and
  Fastly-to-Datadog integration.
- `terraform/`: standalone and older Terraform roots, including DNS, Fastly,
  applications, and team-access configuration.
- `terraform/team-members-datadog/safe-readonly.tf`: source for the Datadog
  permission boundary used by AI agents.
- `ansible/envs/`, `ansible/playbooks/`, and `ansible/roles/`: host inventory,
  environment variables, playbook composition, and server configuration.
- `.github/workflows/main.yml`: repository validation for Terraform formatting,
  Rust code, Fastly Compute code, and bundled actions.

The Fastly-to-Datadog module applies `env`, `app`, and `service` tags. Reuse
those tags to correlate a Fastly service with Datadog telemetry.

Typical searches:

```sh
rg -n '<domain-or-service>' terragrunt terraform ansible
rg -n '<fastly-service-id>' terragrunt terraform
rg -n 'env:|app:|service:' terragrunt/modules terraform
rg -n 'fastly|cloudfront|origin|backend|logging' \
  terragrunt/modules terraform
```

Use README files for architecture intent, then verify every relevant claim in
the actual `.tf`, `.hcl`, YAML, template, or source file.

## Permission-gap handoff

Use this exact structure:

```text
Blocked read-only check:
- Tool and version:
- Command/API:
- Resource and UTC interval:
- Authorization error:
- Current permission relevant to this check:
- Narrowest additional read-only permission/scope needed:
- Evidence this would distinguish:
- Other read-only evidence already checked:
```

Do not include token values, token prefixes beyond their documented token
type, credential paths, or sensitive response bodies.
