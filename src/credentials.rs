use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

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

pub(crate) fn read_secret(reference: &str) -> Result<String> {
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
    Ok(output.trim_end_matches(['\r', '\n']).to_owned())
}

pub(crate) fn install_guest(vm: &str, credentials_name: &str, credentials: &[u8]) -> Result<()> {
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

pub(crate) fn shell_quote(value: &str) -> String {
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
}
