#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::json;
use tempfile::TempDir;

use crate::support::{Machine, Remote, assert_exit};

const KNOWN: &str = r#"{
  "acme": {
    "source": {"source": "github", "repo": "acme/plugins"},
    "installLocation": "/Users/first/.claude/plugins/marketplaces/acme",
    "lastUpdated": "2026-11-01T10:00:00.000Z",
    "autoUpdate": true
  },
  "legacy": {
    "source": {"source": "github", "repo": "team/legacy"},
    "installLocation": "/Users/first/.claude/plugins/marketplaces/legacy"
  }
}
"#;

struct Bin {
    dir: TempDir,
}

impl Bin {
    fn with_fake_claude() -> Self {
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
        let bin = Self { dir };
        let claude = bin.dir.path().join("claude");
        fs::write(
            &claude,
            format!("#!/bin/sh\necho \"$@\" >> '{}'\n", bin.log_file().display()),
        )
        .unwrap();
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
        bin
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

#[test]
fn a_declared_marketplace_reaches_the_other_machine_as_settings_alone() {
    let remote = Remote::new();
    let first = Machine::new();
    let declaration = json!({"acme": {"source": {"source": "github", "repo": "acme/plugins"}, "autoUpdate": true}});
    first.write_settings(&json!({"extraKnownMarketplaces": declaration}));
    first.write_file(".claude/plugins/known_marketplaces.json", KNOWN);
    first.write_file(
        ".claude/plugins/installed_plugins.json",
        r#"{"version": 2, "plugins": {"x@acme": []}}"#,
    );
    first.write_file(".claude/plugins/cache/acme/x/1.0.0/plugin.json", "{}");
    assert_exit(&first.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = Machine::new();
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);
    let bin = Bin::with_fake_claude();

    let output = second.cc_sync_with_path(&bin.path(), &["pull"]);

    assert_exit(&output, 0);
    assert_eq!(bin.log(), "");
    assert_eq!(second.settings()["extraKnownMarketplaces"], declaration);
    let tree =
        String::from_utf8_lossy(&remote.git(&["ls-tree", "-r", "--name-only", "main"]).stdout)
            .into_owned();
    assert!(!tree.contains("plugins/"), "tree: {tree}");
    let objects = remote.all_objects();
    assert!(!objects.contains("installLocation") && !objects.contains("team/legacy"));
}

#[test]
fn status_and_push_warn_about_a_marketplace_that_settings_do_not_declare() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "cc-sync.toml",
        "exclude = [\"settings:/extraKnownMarketplaces/hidden\"]\n",
    );
    let first = Machine::new();
    first.write_settings(&json!({"extraKnownMarketplaces": {"acme": {"source": {"source": "github", "repo": "acme/plugins"}}}}));
    first.write_file(
        ".claude/plugins/known_marketplaces.json",
        &KNOWN.replacen(
            "{\n",
            "{\n  \"hidden\": {\"source\": {\"source\": \"github\", \"repo\": \"team/hidden\"}},\n",
            1,
        ),
    );
    assert_exit(&first.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.cc_sync(&["pull"]), 0);

    let (status, _) = first.cc_sync_json(&["status"]);
    let pushed = first.cc_sync(&["push"]);

    assert_eq!(
        status["undeclared_marketplaces"],
        json!([{"name": "legacy", "declaration": {"source": {"source": "github", "repo": "team/legacy"}}}])
    );
    assert_exit(&pushed, 0);
    let warnings = String::from_utf8_lossy(&pushed.stderr);
    assert!(
        warnings.contains("warning: the marketplace `legacy` does not sync")
            && warnings
                .contains(r#"add "legacy": {"source":{"source":"github","repo":"team/legacy"}}"#)
            && !warnings.contains("hidden"),
        "stderr: {warnings}"
    );
}
