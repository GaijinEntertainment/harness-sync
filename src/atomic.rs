use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(crate) fn write(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    let name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?;
    let tmp = dir.join(format!(
        ".{}.harness-sync-{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = write_then_rename(&tmp, path, contents);
    if result.is_err() {
        // ignored: the write error is the one the caller acts on; a leftover temp file is harmless
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_then_rename(tmp: &Path, path: &Path, contents: &[u8]) -> Result<()> {
    fs::write(tmp, contents).with_context(|| format!("cannot write {}", tmp.display()))?;
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(tmp, meta.permissions())
            .with_context(|| format!("cannot copy the permissions of {}", path.display()))?;
    }
    fs::rename(tmp, path).with_context(|| format!("cannot replace {}", path.display()))
}
