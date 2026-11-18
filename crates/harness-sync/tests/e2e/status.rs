use serde_json::json;

use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn status_before_init_asks_for_init() {
    let machine = Machine::new();

    let output = machine.harness_sync(&["status"]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("harness-sync init"),
        "stderr: {}",
        text(&output.stderr)
    );
}

#[test]
fn status_against_an_empty_remote_lists_every_local_item_as_a_local_change() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high", "env": {"A": "1"}, "model": null}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["status"]);

    assert_exit(&output, 0);
    assert_eq!(report["command"], "status");
    assert_eq!(
        report["items"],
        json!([
            {"id": ".claude/settings.json:/effortLevel", "state": "local_change", "local": "high"},
            {"id": ".claude/settings.json:/env/A", "state": "local_change", "local": "1"},
            {"id": ".claude/settings.json:/model", "state": "local_change", "local": null}
        ])
    );
}

#[test]
fn status_without_a_settings_file_matches_an_empty_remote() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let output = machine.harness_sync(&["status"]);

    assert_exit(&output, 0);
    assert!(
        text(&output.stdout).contains("matches the remote snapshot"),
        "stdout: {}",
        text(&output.stdout)
    );
}

#[test]
fn status_counts_one_difference_in_the_singular() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let output = machine.harness_sync(&["status"]);

    assert_exit(&output, 0);
    assert!(
        text(&output.stdout)
            .starts_with("1 local change, 0 remote changes, 0 conflicts, 0 pinned\n"),
        "stdout: {}",
        text(&output.stdout)
    );
}

#[test]
fn status_reports_a_settings_file_that_is_not_json() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    machine.write_settings(&json!({}));
    std::fs::write(machine.settings_file(), "{ not json").unwrap();

    let output = machine.harness_sync(&["status"]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("is not valid JSON"),
        "stderr: {}",
        text(&output.stderr)
    );
}

#[test]
fn status_lists_and_warns_about_each_unmanaged_file_of_the_remote_snapshot() {
    let remote = Remote::new();
    remote.commit_file("main", "harness-sync.toml", "include = [\".agents\"]\n");
    remote.commit_file("main", ".agents/skills/a.md", "a skill\n");
    remote.commit_file("main", ".pi/agent/models.json", "{}\n");
    remote.commit_file("main", ".claude/settings.json", "{}\n");
    remote.commit_file("main", "README.md", "About this repo.\n");
    remote.commit_file("main", "files/CLAUDE.md", "old layout\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, _) = machine.harness_sync_json(&["status"]);
    let output = machine.harness_sync(&["status"]);

    assert_eq!(
        report["unmanaged_files"],
        json!(["README.md", "files/CLAUDE.md"])
    );
    assert_exit(&output, 0);
    let stderr = text(&output.stderr);
    for file in ["README.md", "files/CLAUDE.md"] {
        assert!(
            stderr.contains(&format!(
                "warning: the remote snapshot holds {file}, which harness-sync does not manage"
            )),
            "stderr: {stderr}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_line_break_in_an_unmanaged_path_does_not_start_a_warning_line() {
    let remote = Remote::new();
    remote.commit_file("main", "a\nwarning: forged", "x\n");
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let output = machine.harness_sync(&["status"]);

    assert_exit(&output, 0);
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("holds a\\nwarning: forged, which"),
        "stderr: {stderr}"
    );
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("warning:"))
            .count(),
        1,
        "stderr: {stderr}"
    );
}
