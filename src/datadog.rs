use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use similar::TextDiff;

use crate::{
    credentials::{install_guest, read_secret, shell_quote},
    guest, snapshot,
};

mod permissions;

const SECRET_REFERENCE: &str = "op://Infrastructure/datadog-read-only/credential";
const PERMISSIONS_SNAPSHOT: &str = "docs/datadog-permissions.json";

const GUEST_TOKEN_DIGEST: &str = r#"
set -Eeuo pipefail
if [[ "${DD_ACCESS_TOKEN:-}" != ddsat_* ]]; then
    echo "The guest does not have a Datadog service access token. Run: just login-datadog" >&2
    exit 1
fi
printf "%s" "$DD_ACCESS_TOKEN" | sha256sum | awk "{print \$1}"
"#;

const CURRENT_USER: &str = r#"
set -Eeuo pipefail
pup api v2/current_user --output json --read-only
"#;

const TOKEN_SCOPES: &str = r#"
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

pub(crate) fn login(vm: &str) -> Result<()> {
    let access_token = read_secret(SECRET_REFERENCE)?;
    if !access_token.starts_with("ddsat_") {
        bail!("Expected a Datadog service access token (prefix: ddsat_).");
    }

    let credentials = format!(
        "export DD_ACCESS_TOKEN={}\nexport DD_SITE={}\n",
        shell_quote(&access_token),
        shell_quote("https://app.datadoghq.com/")
    );
    install_guest(vm, "datadog.env", credentials.as_bytes())?;

    guest::run(vm, "pup monitors list --limit 1 --read-only >/dev/null")?;
    println!("Datadog authentication is configured and working.");
    Ok(())
}

pub(crate) fn dump_permissions(vm: &str) -> Result<()> {
    let path = Path::new(PERMISSIONS_SNAPSHOT);
    assert_current_token(vm)?;
    let live_snapshot = live_permissions(vm)?;
    snapshot::write(path, live_snapshot.as_bytes())?;
    println!(
        "Wrote the current Datadog permissions to {}",
        path.display()
    );
    Ok(())
}

pub(crate) fn assert_permissions(vm: &str) -> Result<()> {
    let path = Path::new(PERMISSIONS_SNAPSHOT);
    assert_current_token(vm)?;

    let expected = fs::read_to_string(path).with_context(|| {
        format!(
            "Datadog permissions snapshot not found: {}\nCreate it with: just dump-datadog-permissions",
            path.display()
        )
    })?;

    let live = live_permissions(vm)?;

    if expected != live {
        let diff = TextDiff::from_lines(&expected, &live)
            .unified_diff()
            .header(&path.display().to_string(), "live Datadog permissions")
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
        path.display()
    );
    Ok(())
}

fn assert_current_token(vm: &str) -> Result<()> {
    let expected_token = read_secret(SECRET_REFERENCE)?;
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

    let guest_digest = guest::capture(vm, GUEST_TOKEN_DIGEST)?;
    if guest_digest.trim_end() != expected_digest {
        bail!(
            "The guest is not using the current Datadog token from 1Password. Run: just login-datadog"
        );
    }
    Ok(())
}

fn live_permissions(vm: &str) -> Result<String> {
    let current_user = guest::capture(vm, CURRENT_USER)?;
    let token_scopes = guest::capture(vm, TOKEN_SCOPES)?;
    let snapshot = permissions::normalize(&current_user, &token_scopes)?;

    let mut json = serde_json::to_string_pretty(&snapshot)
        .context("failed to serialize the Datadog permissions snapshot")?;
    json.push('\n');
    Ok(json)
}
