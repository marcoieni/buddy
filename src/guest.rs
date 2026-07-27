use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub(crate) fn run(vm: &str, script: &str) -> Result<()> {
    let status = Command::new("limactl")
        .args(["shell", vm, "bash", "-lc", script])
        .status()
        .context("failed to run limactl")?;
    if !status.success() {
        bail!("limactl exited with {status}");
    }
    Ok(())
}

pub(crate) fn capture(vm: &str, script: &str) -> Result<String> {
    let output = Command::new("limactl")
        .args(["shell", vm, "bash", "-lc", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run limactl")?;
    if !output.status.success() {
        bail!("limactl exited with {}", output.status);
    }
    String::from_utf8(output.stdout).context("limactl returned non-UTF-8 output")
}
