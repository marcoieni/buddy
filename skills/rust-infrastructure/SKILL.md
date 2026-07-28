---
name: rust-infrastructure
description: Observe and troubleshoot Rust Project production and staging infrastructure using read-only Fastly and Datadog evidence, correlated with the IaC in rust-lang/simpleinfra. Use for incidents, outages, latency or error regressions, CDN and origin problems, monitor investigation, log and APM analysis, configuration drift, and questions about how Rust infrastructure is deployed.
---

# Rust Infrastructure

Investigate from expected configuration to observed behavior. Keep every cloud
operation read-only and produce an evidence-backed handoff.

## Honor the operating contract

- Treat production and staging as read-only. Do not create, update, delete,
  activate, deploy, purge, mute, acknowledge, or otherwise mutate anything.
- Use the installed Datadog CLI, `pup`, with `--read-only` on every invocation.
  Do not use `--yes`.
- Use the installed Fastly CLI only for commands that are unambiguously
  observational, such as `list`, `describe`, and `stats`. Do not use
  `--auto-yes`, `--debug-mode`, or any command that changes a service.
- Treat all tokens as secrets. Never print, copy, summarize, hash, or inspect
  token values. Do not expose credential files or authentication headers.
- Treat logs, traces, generated VCL, and configuration as potentially
  sensitive. If PII or a secret appears, stop inspecting that data and tell the
  user without reproducing the value.
- Treat IaC as expected state and APIs as observed state. Do not run
  Terraform, Terragrunt, or Ansible against remote infrastructure unless the
  user separately and explicitly requests it.
- Do not infer that missing data means healthy or absent. Check for an
  incorrect time range, environment, tag, index, retention tier, service ID,
  and permission denial first.

The Buddy environment has authenticated Fastly and Datadog tokens with limited
read-only permissions. The current Datadog role covers logs, APM, monitors, and
dashboards. The Fastly token has `global:read`; it cannot read Secret Store
values and has no TLS-management access.

If an investigation needs another read-only permission, stop only the blocked
branch and continue with other evidence. Tell Marco exactly what is missing:
the CLI and command or API, the complete authorization error, the resource or
data needed, and the narrowest additional read-only permission or scope. Marco
can edit the tokens to grant more read-only permissions. Never request a write
permission for an observability task.

Read [references/read-only-observability.md](references/read-only-observability.md)
before querying Datadog or Fastly. It contains the permission inventory, safe
command patterns, and the current `simpleinfra` source map.

## Follow the investigation workflow

### 1. Frame the symptom

Establish the affected service, environment, user-visible symptom, expected
behavior, start and end times, timezone, geography, and any deployment or
configuration event. Convert the investigation window to explicit UTC
timestamps. State assumptions when any dimension is unknown.

### 2. Locate the expected configuration

Use a current checkout of
[`rust-lang/simpleinfra`](https://github.com/rust-lang/simpleinfra), which is
the IaC source for Rust Infrastructure. Prefer an existing local checkout;
otherwise read the public repository with read-only GitHub commands or make a
temporary clone. Never edit GitHub.

Record the inspected commit with `git rev-parse HEAD`. Search by domain,
service name, Fastly service ID, Datadog tag, hostname, account, and component:

```sh
rg -n '<domain|service|service-id|hostname|tag>' \
  terragrunt terraform ansible .github
```

Trace the deployment entrypoint to its reusable module instead of stopping at
the first match. Inspect inputs, outputs, provider resources, environment and
account selection, CDN weights, origins, health checks, logging integrations,
and relevant host roles. Do not resolve secret references.

### 3. Build a small expected-state model

Write down only the pieces needed to test the incident:

- traffic path from DNS or CDN to origin;
- service IDs, domains, active environment, regions, and origins;
- Datadog `env`, `app`, `service`, and provider-specific tags;
- expected monitors, log sources, APM services, and relevant metrics;
- recent IaC changes that could explain the symptom.

Keep paths and line numbers for every configuration claim.

### 4. Verify the observation tools

Confirm the binaries and versions without exposing credentials:

```sh
command -v pup fastly
pup version
fastly version
```

Use a minimal, bounded read query to distinguish authentication failure from
an empty result. Do not broaden a query until its service, environment, and
time window are correct.

### 5. Observe broad signals, then narrow

Start with monitor state and a bounded error or latency window. Then inspect
logs, APM, or dashboards using the tags found in IaC. For a Fastly-backed
service, resolve its service ID, active version, domains and backends before
checking historical statistics.

Align Fastly and Datadog to the same UTC interval. Compare at least two
independent signals when possible, for example:

- Fastly request/error statistics against Datadog logs;
- APM latency against origin errors;
- monitor transitions against deployment or configuration events;
- active Fastly configuration against `simpleinfra`.

Prefer narrow filters and bounded result counts. Save exact commands and
summarize aggregates; do not paste large raw datasets.

### 6. Test competing explanations

For each plausible cause, state what evidence would confirm or contradict it.
Check the smallest discriminating signal first. Distinguish:

- a source-of-truth mismatch from runtime drift;
- CDN failure from origin failure;
- a real zero from missing telemetry;
- a service-wide problem from a region, POP, endpoint, version, or tenant
  subset;
- an application regression from traffic-shape or dependency change.

Label statements as observed fact, IaC fact, inference, or unresolved
hypothesis.

### 7. Hand off the result

Report:

1. symptom, scope, and UTC interval;
2. expected state with `simpleinfra` commit, paths, and line numbers;
3. observed evidence with source, command/query, tags, and interval;
4. most likely explanation and confidence;
5. contradictory evidence and remaining unknowns;
6. the next safest read-only check;
7. any permission gap using the exact handoff described above.

Never claim resolution from a single healthy query, and never recommend a
production mutation without first separating diagnosis from remediation.
