use std::{
    ffi::OsStr,
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, ExitCode, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use similar::TextDiff;
use tempfile::Builder;

const DATADOG_SECRET_REFERENCE: &str = "op://Infrastructure/datadog-read-only/credential";
const DATADOG_PERMISSIONS_SNAPSHOT: &str = "docs/datadog-permissions.json";
const FASTLY_SECRET_REFERENCE: &str = "op://Infrastructure/fastly-read-only/credential";

const INSTALL_GUEST_CREDENTIALS: &str = r#"
set -Eeuo pipefail
credentials_dir="$HOME/.config/buddy"
credentials_file="$credentials_dir/$1"

mkdir -p "$credentials_dir"
chmod 700 "$credentials_dir"
umask 077
cat >"$credentials_file"

source_line="[ ! -r \"$credentials_file\" ] || . \"$credentials_file\""
grep -Fqx "$source_line" "$HOME/.profile" ||
    printf "\n%s\n" "$source_line" >>"$HOME/.profile"
"#;

const DATADOG_GUEST_TOKEN_DIGEST: &str = r#"
set -Eeuo pipefail
if [[ "${DD_ACCESS_TOKEN:-}" != ddsat_* ]]; then
    echo "The guest does not have a Datadog service access token. Run: just login-datadog" >&2
    exit 1
fi
printf "%s" "$DD_ACCESS_TOKEN" | sha256sum | awk "{print \$1}"
"#;

const DATADOG_CURRENT_USER: &str = r#"
set -Eeuo pipefail
pup api v2/current_user --output json --read-only
"#;

const DATADOG_TOKEN_SCOPES: &str = r#"
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
"#;

#[derive(Debug, Parser)]
#[command(about = "Manage Buddy VM cloud credentials")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Configure and verify the Datadog credentials in a VM.
    LoginDatadog {
        /// Lima VM name.
        vm: String,
    },
    /// Configure and verify the Fastly credentials in a VM.
    LoginFastly {
        /// Lima VM name.
        vm: String,
    },
    /// Dump or assert the Datadog permissions snapshot.
    DatadogPermissions {
        #[command(subcommand)]
        action: DatadogPermissionsAction,
    },
}

#[derive(Debug, Subcommand)]
enum DatadogPermissionsAction {
    /// Write the VM's current Datadog permissions to a snapshot.
    Dump {
        /// Lima VM name.
        vm: String,
    },
    /// Compare the VM's current Datadog permissions with a snapshot.
    Assert {
        /// Lima VM name.
        vm: String,
    },
}

#[derive(Debug, Serialize)]
struct PermissionsSnapshot {
    schema_version: u8,
    service_account: ServiceAccount,
    token_scopes: Vec<String>,
    roles: Vec<Role>,
    permissions: Vec<Permission>,
}

#[derive(Debug, Serialize)]
struct ServiceAccount {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct Role {
    id: String,
    name: Value,
    receives_permissions_from: Value,
    permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Permission {
    id: String,
    name: Value,
    display_name: Value,
    description: Value,
    group_name: Value,
    name_aliases: Vec<String>,
    restricted: Value,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::LoginDatadog { vm } => login_datadog(&vm),
        Commands::LoginFastly { vm } => login_fastly(&vm),
        Commands::DatadogPermissions { action } => match action {
            DatadogPermissionsAction::Dump { vm } => dump_datadog_permissions(&vm),
            DatadogPermissionsAction::Assert { vm } => assert_datadog_permissions(&vm),
        },
    }
}

fn login_datadog(vm: &str) -> Result<()> {
    let access_token = read_secret(DATADOG_SECRET_REFERENCE)?;
    if !access_token.starts_with("ddsat_") {
        bail!("Expected a Datadog service access token (prefix: ddsat_).");
    }

    let credentials = format!(
        "export DD_ACCESS_TOKEN={}\nexport DD_SITE={}\n",
        shell_quote(&access_token),
        shell_quote("https://app.datadoghq.com/")
    );
    install_guest_credentials(vm, "datadog.env", credentials.as_bytes())?;

    run_guest(vm, "pup monitors list --limit 1 --read-only >/dev/null")?;
    println!("Datadog authentication is configured and working.");
    Ok(())
}

fn login_fastly(vm: &str) -> Result<()> {
    let api_token = read_secret(FASTLY_SECRET_REFERENCE)?;
    if api_token.is_empty() {
        bail!("Expected a non-empty Fastly API token.");
    }

    let credentials = format!(
        "export FASTLY_API_TOKEN={}\nexport FASTLY_DISABLE_AUTH_COMMAND=1\n",
        shell_quote(&api_token)
    );
    install_guest_credentials(vm, "fastly.env", credentials.as_bytes())?;

    run_guest(vm, "fastly service list --per-page 1 >/dev/null")?;
    println!("Fastly authentication is configured and working.");
    Ok(())
}

fn dump_datadog_permissions(vm: &str) -> Result<()> {
    let snapshot = Path::new(DATADOG_PERMISSIONS_SNAPSHOT);
    assert_current_datadog_token(vm)?;
    let live_snapshot = live_datadog_permissions(vm)?;
    write_snapshot(snapshot, live_snapshot.as_bytes())?;
    println!(
        "Wrote the current Datadog permissions to {}",
        snapshot.display()
    );
    Ok(())
}

fn assert_datadog_permissions(vm: &str) -> Result<()> {
    let snapshot = Path::new(DATADOG_PERMISSIONS_SNAPSHOT);
    assert_current_datadog_token(vm)?;

    let expected = fs::read_to_string(snapshot).with_context(|| {
        format!(
            "Datadog permissions snapshot not found: {}\nCreate it with: just dump-datadog-permissions",
            snapshot.display()
        )
    })?;

    let live = live_datadog_permissions(vm)?;

    if expected != live {
        let diff = TextDiff::from_lines(&expected, &live)
            .unified_diff()
            .header(&snapshot.display().to_string(), "live Datadog permissions")
            .to_string();
        print!("{diff}");
        io::stdout()
            .flush()
            .context("failed to print the permissions diff")?;
        bail!(
            "The Datadog identity, roles, or permissions do not match the documented snapshot.\n\
             If the change is intentional, review it with: just dump-datadog-permissions"
        );
    }

    println!(
        "Datadog credentials are current and their permissions match {}",
        snapshot.display()
    );
    Ok(())
}

fn assert_current_datadog_token(vm: &str) -> Result<()> {
    let expected_token = read_secret(DATADOG_SECRET_REFERENCE)?;
    if !expected_token.starts_with("ddsat_") {
        bail!(
            "Expected the 1Password item to contain a Datadog service access token (prefix: ddsat_)."
        );
    }

    let expected_digest = Sha256::digest(expected_token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    drop(expected_token);

    let guest_digest = run_guest_capture(vm, DATADOG_GUEST_TOKEN_DIGEST)?;
    if guest_digest.trim_end() != expected_digest {
        bail!(
            "The guest is not using the current Datadog token from 1Password. Run: just login-datadog"
        );
    }
    Ok(())
}

fn live_datadog_permissions(vm: &str) -> Result<String> {
    let current_user = run_guest_capture(vm, DATADOG_CURRENT_USER)?;
    let token_scopes = run_guest_capture(vm, DATADOG_TOKEN_SCOPES)?;
    let snapshot = normalize_permissions(&current_user, &token_scopes)?;

    let mut json = serde_json::to_string_pretty(&snapshot)
        .context("failed to serialize the Datadog permissions snapshot")?;
    json.push('\n');
    Ok(json)
}

fn normalize_permissions(
    current_user_json: &str,
    token_scopes_json: &str,
) -> Result<PermissionsSnapshot> {
    let response: Value = serde_json::from_str(current_user_json)
        .context("current_user did not return valid JSON")?;
    let token: Value = serde_json::from_str(token_scopes_json)
        .context("the access-token query did not return valid JSON")?;

    let service_account = ServiceAccount {
        id: string_at(&response, "/data/id")
            .context("current_user did not return a service-account identity")?,
        name: string_at(&response, "/data/attributes/name")
            .context("current_user did not return a service-account identity")?,
    };

    let mut token_scopes = string_array_at(&token, "/scopes")
        .context("the service access token did not return scopes")?;
    token_scopes.sort();
    if token_scopes.is_empty() {
        bail!("the service access token did not return any scopes");
    }

    let included = response
        .get("included")
        .and_then(Value::as_array)
        .context("current_user did not return included roles and permissions")?;

    let mut roles = Vec::new();
    let mut permissions = Vec::new();

    for item in included {
        match item.get("type").and_then(Value::as_str) {
            Some("roles") => roles.push(normalize_role(item)?),
            Some("permissions") => permissions.push(normalize_permission(item)?),
            _ => {}
        }
    }

    roles.sort_by(|left, right| left.id.cmp(&right.id));
    permissions.sort_by(|left, right| left.id.cmp(&right.id));

    if roles.is_empty() {
        bail!("current_user did not return any roles");
    }
    if permissions.is_empty() {
        bail!("current_user did not return any permissions");
    }

    Ok(PermissionsSnapshot {
        schema_version: 1,
        service_account,
        token_scopes,
        roles,
        permissions,
    })
}

fn normalize_role(item: &Value) -> Result<Role> {
    let mut permissions = string_array_at(item, ("/relationships/permissions/data", "/id"))
        .context("a Datadog role returned invalid permissions")?;
    permissions.sort();

    Ok(Role {
        id: string_at(item, "/id").context("a Datadog role has no ID")?,
        name: value_at(item, "/attributes/name"),
        receives_permissions_from: value_at(item, "/attributes/receives_permissions_from"),
        permissions,
    })
}

fn normalize_permission(item: &Value) -> Result<Permission> {
    let mut name_aliases = optional_string_array_at(item, "/attributes/name_aliases")
        .context("a Datadog permission returned invalid name aliases")?;
    name_aliases.sort();

    Ok(Permission {
        id: string_at(item, "/id").context("a Datadog permission has no ID")?,
        name: value_at(item, "/attributes/name"),
        display_name: value_at(item, "/attributes/display_name"),
        description: value_at(item, "/attributes/description"),
        group_name: value_at(item, "/attributes/group_name"),
        name_aliases,
        restricted: value_at(item, "/attributes/restricted"),
    })
}

fn string_at(value: &Value, pointer: &str) -> Option<String> {
    value.pointer(pointer)?.as_str().map(str::to_owned)
}

trait StringArrayAt {
    fn read(self, value: &Value) -> Result<Vec<String>>;
}

impl StringArrayAt for &str {
    fn read(self, value: &Value) -> Result<Vec<String>> {
        let array = value
            .pointer(self)
            .and_then(Value::as_array)
            .with_context(|| format!("expected an array at {self}"))?;
        array
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .map(str::to_owned)
                    .with_context(|| format!("expected a string in {self}"))
            })
            .collect()
    }
}

impl StringArrayAt for (&str, &str) {
    fn read(self, value: &Value) -> Result<Vec<String>> {
        let (array_pointer, item_pointer) = self;
        let array = match value.pointer(array_pointer) {
            None | Some(Value::Null) => return Ok(Vec::new()),
            Some(array) => array,
        };
        let array = array
            .as_array()
            .with_context(|| format!("expected an array at {array_pointer}"))?;
        array
            .iter()
            .map(|entry| {
                string_at(entry, item_pointer).with_context(|| {
                    format!("expected a string at {array_pointer}[]{item_pointer}")
                })
            })
            .collect()
    }
}

fn string_array_at<A: StringArrayAt>(value: &Value, pointer: A) -> Result<Vec<String>> {
    pointer.read(value)
}

fn optional_string_array_at(value: &Value, pointer: &str) -> Result<Vec<String>> {
    match value.pointer(pointer) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(_) => string_array_at(value, pointer),
    }
}

fn value_at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn read_secret(reference: &str) -> Result<String> {
    let mut command = Command::new("op");
    command.args(["read", reference]);
    let output = capture_stdout(&mut command, "op")?;
    Ok(output.trim_end_matches(['\r', '\n']).to_owned())
}

fn install_guest_credentials(vm: &str, credentials_name: &str, credentials: &[u8]) -> Result<()> {
    if Path::new(credentials_name).file_name() != Some(OsStr::new(credentials_name)) {
        bail!("Expected a credentials filename, not a path.");
    }

    let mut child = Command::new("limactl")
        .args([
            "shell",
            vm,
            "bash",
            "-c",
            INSTALL_GUEST_CREDENTIALS,
            "bash",
            credentials_name,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to run limactl")?;

    let write_result = child
        .stdin
        .take()
        .context("failed to open limactl stdin")?
        .write_all(credentials);
    let status = child.wait().context("failed to wait for limactl")?;

    if !status.success() {
        bail!("limactl exited with {status}");
    }
    write_result.context("failed to send credentials to the guest")
}

fn run_guest(vm: &str, script: &str) -> Result<()> {
    let status = Command::new("limactl")
        .args(["shell", vm, "bash", "-lc", script])
        .status()
        .context("failed to run limactl")?;
    if !status.success() {
        bail!("limactl exited with {status}");
    }
    Ok(())
}

fn run_guest_capture(vm: &str, script: &str) -> Result<String> {
    let mut command = Command::new("limactl");
    command.args(["shell", vm, "bash", "-lc", script]);
    capture_stdout(&mut command, "limactl")
}

fn capture_stdout(command: &mut Command, program: &str) -> Result<String> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        bail!("{program} exited with {}", output.status);
    }
    String::from_utf8(output.stdout).with_context(|| format!("{program} returned non-UTF-8 output"))
}

fn write_snapshot(snapshot: &Path, contents: &[u8]) -> Result<()> {
    let parent = snapshot
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create snapshot directory {}", parent.display()))?;

    let prefix = snapshot
        .file_name()
        .and_then(OsStr::to_str)
        .map(|name| format!("{name}.tmp."))
        .unwrap_or_else(|| "datadog-permissions.tmp.".to_owned());
    let mut temporary = Builder::new()
        .prefix(&prefix)
        .tempfile_in(parent)
        .with_context(|| format!("failed to create a temporary file in {}", parent.display()))?;

    temporary
        .write_all(contents)
        .context("failed to write the permissions snapshot")?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))
        .context("failed to set permissions on the permissions snapshot")?;
    temporary
        .persist(snapshot)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "failed to replace permissions snapshot {}",
                snapshot.display()
            )
        })?;
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_USER: &str = r#"
    {
      "data": {
        "id": "service-account-1",
        "attributes": {"name": "Buddy"}
      },
      "included": [
        {
          "type": "permissions",
          "id": "permission-b",
          "attributes": {
            "name": "b",
            "display_name": "B",
            "description": null,
            "group_name": "group",
            "name_aliases": ["z", "a"],
            "restricted": false
          }
        },
        {
          "type": "roles",
          "id": "role-1",
          "attributes": {
            "name": "Read only",
            "receives_permissions_from": []
          },
          "relationships": {
            "permissions": {
              "data": [{"id": "permission-b"}, {"id": "permission-a"}]
            }
          }
        },
        {
          "type": "permissions",
          "id": "permission-a",
          "attributes": {
            "name": "a",
            "display_name": "A",
            "description": "description",
            "group_name": "group",
            "restricted": true
          }
        }
      ]
    }
    "#;

    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("a b'$HOME"), "'a b'\\''$HOME'");
    }

    #[test]
    fn normalizes_and_sorts_permissions() {
        let snapshot = normalize_permissions(CURRENT_USER, r#"{"scopes":["z","a"]}"#).unwrap();
        let json = serde_json::to_value(snapshot).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["service_account"]["id"], "service-account-1");
        assert_eq!(json["token_scopes"], serde_json::json!(["a", "z"]));
        assert_eq!(json["permissions"][0]["id"], "permission-a");
        assert_eq!(
            json["permissions"][0]["name_aliases"],
            serde_json::json!([])
        );
        assert_eq!(
            json["permissions"][1]["name_aliases"],
            serde_json::json!(["a", "z"])
        );
        assert_eq!(
            json["roles"][0]["permissions"],
            serde_json::json!(["permission-a", "permission-b"])
        );
    }

    #[test]
    fn rejects_empty_roles() {
        let current_user = r#"
        {
          "data": {"id": "service-account-1", "attributes": {"name": "Buddy"}},
          "included": [
            {
              "type": "permissions",
              "id": "permission-1",
              "attributes": {"name_aliases": []}
            }
          ]
        }
        "#;

        let error = normalize_permissions(current_user, r#"{"scopes":["read"]}"#).unwrap_err();
        assert_eq!(error.to_string(), "current_user did not return any roles");
    }
}
