use std::io::{self, ErrorKind, Write};

use anyhow::Result;

use crate::report::Report;

pub(crate) fn report(report: &Report, json: bool) -> Result<()> {
    let mut text = if json {
        serde_json::to_string_pretty(report)?
    } else {
        report.summary.clone()
    };
    text.push('\n');
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(err) if err.kind() != ErrorKind::BrokenPipe => Err(err.into()),
        _ => Ok(()),
    }
}

pub(crate) fn error(err: &anyhow::Error) {
    // ignored: stderr is the last channel left to report on
    let _ = writeln!(io::stderr().lock(), "error: {err:#}");
}
