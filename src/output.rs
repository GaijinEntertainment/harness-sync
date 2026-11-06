use std::fmt::Write as _;
use std::io::{self, ErrorKind, IsTerminal, Write};

use anyhow::Result;

use crate::compare::ItemState;
use crate::item::KeyPath;
use crate::report::Report;

pub(crate) fn report(report: &Report, json: bool) -> Result<()> {
    let text = if json {
        serde_json::to_string_pretty(report)? + "\n"
    } else {
        human(report)
    };
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(err) if err.kind() != ErrorKind::BrokenPipe => Err(err.into()),
        _ => Ok(()),
    }
}

pub(crate) fn can_prompt(no_input: bool) -> bool {
    !no_input && io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub(crate) fn ask_secret(key: &KeyPath) -> Result<Option<String>> {
    let value = rpassword::prompt_password(format!("value of the secret {key} (empty to skip): "))?;
    Ok(Some(value).filter(|value| !value.is_empty()))
}

pub(crate) fn error(err: &anyhow::Error) {
    // ignored: stderr is the last channel left to report on
    let _ = writeln!(io::stderr().lock(), "error: {err:#}");
}

fn human(report: &Report) -> String {
    let mut text = format!("{}\n", report.summary);
    for item in &report.items {
        // ignored: writing to a String cannot fail
        let _ = writeln!(text, "  {:<14}  {}", label(item.state), item.id);
    }
    text
}

const fn label(state: ItemState) -> &'static str {
    match state {
        ItemState::LocalChange => "local change",
        ItemState::RemoteChange => "remote change",
        ItemState::Conflict => "conflict",
        ItemState::Pinned => "pinned",
        ItemState::SecretMissing => "secret missing",
    }
}
