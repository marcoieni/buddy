use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use reqwest::{
    Client,
    header::{ACCEPT, HeaderValue},
    redirect::Policy,
};
use serde::Deserialize;

use crate::{
    credentials::{install_guest_env, read_secret},
    guest,
};

const SECRET_REFERENCE: &str = "op://Infrastructure/fastly-read-only/credential";
const API_BASE_URL: &str = "https://api.fastly.com";
const REQUIRED_SCOPE: &str = "global:read";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) async fn login(vm: &str) -> anyhow::Result<()> {
    let api_token = read_secret(SECRET_REFERENCE)?;
    if api_token.is_empty() {
        bail!("Expected a non-empty Fastly API token.");
    }
    if api_token.contains(['\r', '\n']) {
        bail!("Expected the Fastly API token to contain no line breaks.");
    }

    assert_token_metadata(&api_token).await?;

    install_guest_env(
        vm,
        "fastly.env",
        &[
            ("FASTLY_API_TOKEN", &api_token),
            ("FASTLY_DISABLE_AUTH_COMMAND", "1"),
        ],
    )?;

    // Test that Fastly authentication is working.
    guest::capture(vm, "fastly service list --per-page 1")?;

    println!("Fastly authentication is configured and working.");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct TokenMetadata {
    scope: String,
    expires_at: Option<String>,
}

#[derive(Debug)]
struct ApiResponse {
    status: u16,
    body: String,
}

/// Verifies the token metadata available from Fastly's `/tokens/self` endpoint.
///
/// This checks that the token has exactly the read-only scope Buddy requires and
/// that it has not expired. It cannot verify that the token is an automation
/// token or that TLS management is disabled: Fastly exposes those properties
/// through `/automation-tokens/{id}`, which requires account-management
/// permissions that the token's `User` role intentionally lacks. Checking them
/// would require a separate, privileged credential.
async fn assert_token_metadata(api_token: &str) -> anyhow::Result<()> {
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("failed to create Fastly API client")?;
    let current_response = api_request(&client, api_token, "/tokens/self").await?;
    require_success(
        &current_response,
        "retrieve the current Fastly token metadata",
    )?;

    let metadata: TokenMetadata = serde_json::from_str(&current_response.body)
        .context("Fastly returned invalid current token metadata")?;

    validate_current_token(&metadata, SystemTime::now().into())
}

fn validate_current_token(metadata: &TokenMetadata, now: DateTime<Utc>) -> anyhow::Result<()> {
    if metadata.scope != REQUIRED_SCOPE {
        bail!(
            "Expected the Fastly token to have exactly the {REQUIRED_SCOPE:?} scope, got {:?}.",
            metadata.scope
        );
    }

    let expires_at = metadata
        .expires_at
        .as_deref()
        .context("Expected the Fastly token to have an expiration date.")?;
    let expires_at = DateTime::parse_from_rfc3339(expires_at)
        .context("Fastly returned an invalid token expiration date")?
        .with_timezone(&Utc);
    if expires_at <= now {
        bail!("Expected the Fastly token expiration date to be in the future, got {expires_at}.");
    }

    Ok(())
}

async fn api_request(client: &Client, api_token: &str, path: &str) -> anyhow::Result<ApiResponse> {
    let url = format!("{API_BASE_URL}{path}");
    let mut api_token =
        HeaderValue::from_str(api_token).context("Fastly API token is not a valid header value")?;
    api_token.set_sensitive(true);
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .header("Fastly-Key", api_token)
        .send()
        .await
        .context("failed to request the Fastly API")?;
    let status = response.status().as_u16();
    let body = String::from_utf8(
        response
            .bytes()
            .await
            .context("failed to read the Fastly API response")?
            .to_vec(),
    )
    .context("Fastly returned a non-UTF-8 response")?;

    Ok(ApiResponse { status, body })
}

fn require_success(response: &ApiResponse, action: &str) -> anyhow::Result<()> {
    if !(200..300).contains(&response.status) {
        bail!(
            "Failed to {action}: Fastly returned HTTP {}: {}",
            response.status,
            response.body.trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn metadata(scope: &str, expires_at: Option<&str>) -> TokenMetadata {
        TokenMetadata {
            scope: scope.to_owned(),
            expires_at: expires_at.map(str::to_owned),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap()
    }

    #[test]
    fn accepts_exact_read_only_scope_and_future_expiration() {
        validate_current_token(
            &metadata(REQUIRED_SCOPE, Some("2026-07-28T14:00:01+01:00")),
            now(),
        )
        .unwrap();
    }

    #[test]
    fn rejects_other_or_additional_scopes() {
        for scope in ["global", "purge_select", "global:read purge_select"] {
            let error =
                validate_current_token(&metadata(scope, Some("2026-07-29T00:00:00Z")), now())
                    .unwrap_err()
                    .to_string();
            assert!(
                error.contains("exactly the \"global:read\" scope"),
                "{error:?}"
            );
        }
    }

    #[test]
    fn rejects_missing_expiration() {
        assert_eq!(
            validate_current_token(&metadata(REQUIRED_SCOPE, None), now())
                .unwrap_err()
                .to_string(),
            "Expected the Fastly token to have an expiration date."
        );
    }

    #[test]
    fn rejects_expiration_that_is_not_in_the_future() {
        for expires_at in ["2026-07-28T11:59:59Z", "2026-07-28T12:00:00Z"] {
            let error = validate_current_token(&metadata(REQUIRED_SCOPE, Some(expires_at)), now())
                .unwrap_err()
                .to_string();
            assert!(error.contains("to be in the future"), "{error:?}");
        }
    }

    #[test]
    fn rejects_invalid_expiration() {
        let error =
            validate_current_token(&metadata(REQUIRED_SCOPE, Some("tomorrow")), now()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Fastly returned an invalid token expiration date"
        );
    }
}
