use anyhow::bail;

use crate::{
    credentials::{install_guest_env, read_secret},
    guest,
};

const SECRET_REFERENCE: &str = "op://Infrastructure/fastly-read-only/credential";

pub(crate) fn login(vm: &str) -> anyhow::Result<()> {
    let api_token = read_secret(SECRET_REFERENCE)?;
    if api_token.is_empty() {
        bail!("Expected a non-empty Fastly API token.");
    }

    install_guest_env(
        vm,
        "fastly.env",
        &[
            ("FASTLY_API_TOKEN", &api_token),
            ("FASTLY_DISABLE_AUTH_COMMAND", "1"),
        ],
    )?;

    guest::run(vm, "fastly service list --per-page 1")?;
    println!("Fastly authentication is configured and working.");
    Ok(())
}
