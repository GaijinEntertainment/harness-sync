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
    machine.write_settings(&json!({"effortLevel": "high", "env": {"A": "1"}}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["status"]);

    assert_exit(&output, 0);
    assert_eq!(report["command"], "status");
    assert_eq!(
        report["items"],
        json!([
            {"id": ".claude/settings.json:/effortLevel", "state": "local_change", "local": "high", "remote": null},
            {"id": ".claude/settings.json:/env/A", "state": "local_change", "local": "1", "remote": null}
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
