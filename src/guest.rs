use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

use anyhow::{Context, bail};

// Run a command capturing stdout
pub(crate) fn capture(vm: &str, script: &str) -> anyhow::Result<String> {
    let output = Command::new("limactl")
        .args(["shell", vm, "bash", "-lc", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to run limactl")?;
    if !output.status.success() {
        io::stdout()
            .write_all(&output.stdout)
            .context("failed to relay limactl stdout")?;
        io::stdout()
            .flush()
            .context("failed to flush limactl stdout")?;
        bail!("limactl exited with {}", output.status);
    }
    String::from_utf8(output.stdout).context("limactl returned non-UTF-8 output")
}
