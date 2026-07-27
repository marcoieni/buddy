use std::{ffi::OsStr, fs, io::Write, os::unix::fs::PermissionsExt, path::Path};

use anyhow::Context;
use tempfile::Builder;

pub(crate) fn write(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create snapshot directory {parent:?}"))?;

    let prefix = path
        .file_name()
        .and_then(OsStr::to_str)
        .map(|name| format!("{name}.tmp."))
        .unwrap_or_else(|| "datadog-permissions.tmp.".to_owned());
    let mut temporary = Builder::new()
        .prefix(&prefix)
        .tempfile_in(parent)
        .with_context(|| format!("failed to create a temporary file in {parent:?}"))?;

    temporary
        .write_all(contents)
        .context("failed to write the permissions snapshot")?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))
        .context("failed to set permissions on the permissions snapshot")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace permissions snapshot {path:?}"))?;
    Ok(())
}
