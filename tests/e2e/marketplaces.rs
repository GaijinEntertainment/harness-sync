#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::support::{Machine, Remote, assert_exit};

const KNOWN: &str = r#"{
  "acme": {
    "source": {"source": "github", "repo": "acme/plugins"},
    "installLocation": "/Users/first/.claude/plugins/marketplaces/acme",
    "lastUpdated": "2026-11-01T10:00:00.000Z"
  },
  "declared": {
    "source": {"source": "github", "repo": "team/declared"},
    "installLocation": "/Users/first/.claude/plugins/marketplaces/declared"
  }
}
"#;

struct Bin {
    dir: TempDir,
}

impl Bin {
    fn with_fake_claude() -> Self {
        let bin = Self::new();
        let claude = bin.dir.path().join("claude");
        let log = bin.log_file();
        fs::write(
            &claude,
            format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
        )
        .unwrap();
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let git = String::from_utf8(
            std::process::Command::new("sh")
                .args(["-c", "command -v git"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        std::os::unix::fs::symlink(git.trim(), dir.path().join("git")).unwrap();
        Self { dir }
    }

    fn path(&self) -> OsString {
        self.dir.path().as_os_str().to_owned()
    }

    fn log_file(&self) -> std::path::PathBuf {
        self.dir.path().join("claude.log")
    }

    fn log(&self) -> String {
        fs::read_to_string(self.log_file()).unwrap_or_default()
    }
}

fn first_with_marketplaces(remote: &Remote) -> Machine {
    let first = Machine::new();
    first.write_settings(&json!({"extraKnownMarketplaces": {"declared": {"source": {"source": "github", "repo": "team/declared"}}}}));
    first.write_file(".claude/plugins/known_marketplaces.json", KNOWN);
    first.write_file(
        ".claude/plugins/installed_plugins.json",
        r#"{"version": 2, "plugins": {"x@acme": []}}"#,
    );
    first.write_file(".claude/plugins/cache/acme/x/1.0.0/plugin.json", "{}");
    assert_exit(&first.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.cc_sync(&["push"]), 0);
    first
}

fn tree(remote: &Remote) -> String {
    String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
        .into_owned()
}

#[test]
fn a_marketplace_added_on_one_machine_is_added_on_the_other_through_the_claude_cli() {
    let remote = Remote::new();
    first_with_marketplaces(&remote);
    let second = Machine::new();
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);
    let bin = Bin::with_fake_claude();

    let output = second.cc_sync_with_path(&bin.path(), &["pull"]);

    assert_exit(&output, 0);
    assert_eq!(bin.log(), "plugin marketplace add acme/plugins\n");
    let marketplaces: Value =
        serde_json::from_str(&remote.show("main:plugins/marketplaces.json")).unwrap();
    assert_eq!(
        marketplaces,
        json!({
            "acme": {"source": {"source": "github", "repo": "acme/plugins"}},
            "declared": {"source": {"source": "github", "repo": "team/declared"}}
        })
    );
    let tree = tree(&remote);
    assert!(
        !tree.contains("installed_plugins") && !tree.contains("cache"),
        "tree: {tree}"
    );
    assert!(!remote.all_objects().contains("installLocation"));
}

#[test]
fn pull_without_the_claude_cli_keeps_the_base_commit() {
    let remote = Remote::new();
    first_with_marketplaces(&remote);
    let second = Machine::new();
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);
    let bin = Bin::new();

    let output = second.cc_sync_with_path(&bin.path(), &["pull"]);

    assert_exit(&output, 1);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("marketplace:acme: the `claude` command is not on PATH")
    );
    assert_exit(&second.cc_sync_with_path(&bin.path(), &["push"]), 4);
    assert_eq!(
        second.settings()["extraKnownMarketplaces"]["declared"]["source"]["repo"],
        "team/declared"
    );
}

#[test]
fn a_marketplace_that_moves_into_settings_is_never_removed() {
    let remote = Remote::new();
    let first = first_with_marketplaces(&remote);
    let second = Machine::new();
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(
        &second.cc_sync_with_path(&Bin::with_fake_claude().path(), &["pull"]),
        0,
    );
    second.write_settings(&json!({"extraKnownMarketplaces": {
        "declared": {"source": {"source": "github", "repo": "team/declared"}},
        "acme": {"source": {"source": "github", "repo": "acme/plugins"}}
    }}));
    second.write_file(".claude/plugins/known_marketplaces.json", KNOWN);
    assert_exit(&second.cc_sync(&["push"]), 0);
    let bin = Bin::with_fake_claude();

    let output = first.cc_sync_with_path(&bin.path(), &["pull"]);

    assert_exit(&output, 0);
    assert_eq!(bin.log(), "");
    assert_eq!(
        first.settings()["extraKnownMarketplaces"]["acme"]["source"]["repo"],
        "acme/plugins"
    );
}
