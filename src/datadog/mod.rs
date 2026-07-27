use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use similar::TextDiff;

use crate::{
    credentials::{install_guest_env, read_secret},
    guest,
};

mod permissions;

const SECRET_REFERENCE: &str = "op://Infrastructure/datadog-read-only/credential";
const PERMISSIONS_SNAPSHOT: &str = "docs/datadog-permissions.json";

const GUEST_TOKEN_DIGEST: &str = include_str!("../../scripts/datadog-guest-token-digest.sh");
const CURRENT_USER: &str = include_str!("../../scripts/datadog-current-user.sh");

pub(crate) fn login(vm: &str) -> anyhow::Result<()> {
    let access_token = read_secret(SECRET_REFERENCE)?;
    if !access_token.starts_with("ddsat_") {
        bail!("Expected a Datadog service access token (prefix: ddsat_).");
    }

    install_guest_env(
        vm,
        "datadog.env",
        &[
            ("DD_ACCESS_TOKEN", &access_token),
            ("DD_SITE", "https://app.datadoghq.com/"),
        ],
    )?;

    guest::capture(vm, "pup monitors list --limit 1 --read-only")?;
    println!("Datadog authentication is configured and working.");
    Ok(())
}

pub(crate) fn dump_permissions(vm: &str) -> anyhow::Result<()> {
    let path = Path::new(PERMISSIONS_SNAPSHOT);
    assert_current_token(vm)?;
    let live_snapshot = live_permissions(vm)?;
    fs::write(path, live_snapshot)
        .with_context(|| format!("failed to write permissions snapshot {path:?}"))?;
    println!("Wrote the current Datadog permissions to {path:?}");
    Ok(())
}

pub(crate) fn assert_permissions(vm: &str) -> anyhow::Result<()> {
    let path = Path::new(PERMISSIONS_SNAPSHOT);
    assert_current_token(vm)?;

    let expected = fs::read_to_string(path).with_context(|| {
        format!(
            "Datadog permissions snapshot not found: {path:?}\nCreate it with: just dump-datadog-permissions"
        )
    })?;

    let live = live_permissions(vm)?;

    if expected != live {
        let diff = TextDiff::from_lines(&expected, &live)
            .unified_diff()
            .header(&format!("{path:?}"), "live Datadog permissions")
            .to_string();
        print!("{diff}");
        io::stdout()
            .flush()
            .context("failed to print the permissions diff")?;
        bail!(
            "The Datadog permissions do not match the documented snapshot.\n\
             If the change is intentional, review it with: just dump-datadog-permissions"
        );
    }

    println!("Datadog credentials are current and their permissions match {path:?}");
    Ok(())
}

fn assert_current_token(vm: &str) -> anyhow::Result<()> {
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

fn live_permissions(vm: &str) -> anyhow::Result<String> {
    let current_user = guest::capture(vm, CURRENT_USER)?;
    let snapshot = permissions::normalize(&current_user)?;

    let mut json = serde_json::to_string_pretty(&snapshot)
        .context("failed to serialize the Datadog permissions snapshot")?;
    json.push('\n');
    Ok(json)
}
