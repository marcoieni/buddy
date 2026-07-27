use anyhow::{Result, bail};

use crate::{
    credentials::{install_guest, read_secret, shell_quote},
    guest,
};

const SECRET_REFERENCE: &str = "op://Infrastructure/fastly-read-only/credential";

pub(crate) fn login(vm: &str) -> Result<()> {
    let api_token = read_secret(SECRET_REFERENCE)?;
    if api_token.is_empty() {
        bail!("Expected a non-empty Fastly API token.");
    }

    let credentials = format!(
        "export FASTLY_API_TOKEN={}\nexport FASTLY_DISABLE_AUTH_COMMAND=1\n",
        shell_quote(&api_token)
    );
    install_guest(vm, "fastly.env", credentials.as_bytes())?;

    guest::run(vm, "fastly service list --per-page 1 >/dev/null")?;
    println!("Fastly authentication is configured and working.");
    Ok(())
}
