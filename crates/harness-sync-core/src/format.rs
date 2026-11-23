//! The format numbers of the repo and of the machine-local state (ADR 1.20).

use anyhow::{Result, bail, ensure};
use toml::{Table, Value};

pub(crate) const REPO: u32 = 1;
pub(crate) const STATE: u32 = 1;

/// The format of a file that holds no `format` key.
pub(crate) const fn first() -> u32 {
    1
}

/// Checks the `format` of `file` before anything reads its other keys, whose shape a newer format can change.
pub(crate) fn ensure_readable(file: &Table, known: u32, holder: &str) -> Result<()> {
    let format = match file.get("format") {
        None => i64::from(first()),
        Some(Value::Integer(format)) => *format,
        Some(_) => bail!("the `format` of {holder} is not a number"),
    };
    ensure!(
        format <= i64::from(known),
        "{holder} has the format {format}, and this harness-sync reads the formats up to {known}: install a newer \
         harness-sync"
    );
    Ok(())
}
