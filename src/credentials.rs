use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, bail};

const INSTALL_GUEST_CREDENTIALS: &str = include_str!("../scripts/install-guest-credentials.sh");

pub(crate) fn read_secret(reference: &str) -> anyhow::Result<String> {
    let output = Command::new("op")
        .args(["read", reference])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run op")?;
    if !output.status.success() {
        bail!("op exited with {}", output.status);
    }

    let output = String::from_utf8(output.stdout).context("op returned non-UTF-8 output")?;
    Ok(output.trim().to_owned())
}

/// Installs credentials in the guest's Buddy config directory.
///
/// The credentials are sent over stdin so they never appear in the `limactl`
/// arguments. The bundled script creates the destination with restrictive
/// permissions and adds it to the guest's shell profile.
fn install_guest(vm: &str, credentials_name: &str, credentials: &[u8]) -> anyhow::Result<()> {
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

pub(crate) fn install_guest_env(
    vm: &str,
    credentials_name: &str,
    environment: &[(&str, &str)],
) -> anyhow::Result<()> {
    let credentials = serialize_env(environment)?;
    install_guest(vm, credentials_name, credentials.as_bytes())
}

fn serialize_env(environment: &[(&str, &str)]) -> anyhow::Result<String> {
    let mut credentials = String::new();
    for &(name, value) in environment {
        if !is_valid_env_name(name) {
            bail!("Invalid environment variable name: {name:?}");
        }
        credentials.push_str("export ");
        credentials.push_str(name);
        credentials.push('=');
        credentials.push_str(&shell_quote(value));
        credentials.push('\n');
    }

    Ok(credentials)
}

fn is_valid_env_name(name: &str) -> bool {
    let mut characters = name.bytes();
    matches!(characters.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && characters.all(|character| character.is_ascii_alphanumeric() || character == b'_')
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("a b'$HOME"), "'a b'\\''$HOME'");
    }

    #[test]
    fn validates_environment_variable_names() {
        for name in ["A", "_", "DD_ACCESS_TOKEN", "value2"] {
            assert!(is_valid_env_name(name), "{name:?} should be valid");
        }
        for name in ["", "2_VALUE", "A-B", "A=B", "A B", "A\nB"] {
            assert!(!is_valid_env_name(name), "{name:?} should be invalid");
        }
    }

    #[test]
    fn serializes_environment_variables() {
        assert_eq!(
            serialize_env(&[("TOKEN", "a b'$HOME"), ("ENABLED", "1")]).unwrap(),
            "export TOKEN='a b'\\''$HOME'\nexport ENABLED='1'\n"
        );
        assert_eq!(
            serialize_env(&[("TOKEN; echo injected", "secret")])
                .unwrap_err()
                .to_string(),
            "Invalid environment variable name: \"TOKEN; echo injected\""
        );
    }
}
