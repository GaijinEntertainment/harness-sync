use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn init_connects_to_an_empty_remote() {
    let remote = Remote::new();
    let machine = Machine::new();

    let (report, output) = machine.harness_sync_json(&["init", &remote.url()]);

    assert_exit(&output, 0);
    assert_eq!(report["command"], "init");
    assert_eq!(report["outcome"], "ok");
    assert!(report["remote_commit"].is_null());
    let config = std::fs::read_to_string(machine.home().join(".harness-sync/config.toml")).unwrap();
    assert!(config.contains(&remote.url()), "config: {config}");
}

#[test]
fn init_with_the_same_remote_again_succeeds() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let output = machine.harness_sync(&["init", &remote.url()]);

    assert_exit(&output, 0);
}

#[test]
fn init_with_another_remote_is_refused() {
    let first = Remote::new();
    let second = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &first.url()]), 0);

    let output = machine.harness_sync(&["init", &second.url()]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("already connected"),
        "stderr: {}",
        text(&output.stderr)
    );
}

#[test]
fn init_syncs_each_harness_whose_directory_exists() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["harness", "list"]);

    assert_exit(&output, 0);
    assert_eq!(
        report["harnesses"],
        serde_json::json!([
            {"name": "claude-code", "synced": true},
            {"name": "pi", "synced": false}
        ])
    );
}

#[test]
fn init_without_a_harness_directory_needs_the_harnesses_named() {
    let remote = Remote::new();
    let machine = Machine::without_harnesses();

    let refused = machine.harness_sync(&["init", &remote.url()]);
    let unknown = machine.harness_sync(&["init", &remote.url(), "--harness", "vim"]);
    let named = machine.harness_sync(&["init", &remote.url(), "--harness", "claude-code"]);

    assert_exit(&refused, 1);
    assert!(text(&refused.stderr).contains("--harness"));
    assert_exit(&unknown, 1);
    assert!(text(&unknown.stderr).contains("claude-code"));
    assert_exit(&named, 0);
}

#[test]
fn init_with_an_unreachable_remote_fails_without_a_config() {
    let machine = Machine::new();
    let missing = machine.home().join("no-such-remote.git");

    let output = machine.harness_sync(&["init", &missing.to_string_lossy()]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("git fetch failed"),
        "stderr: {}",
        text(&output.stderr)
    );
    assert!(!machine.home().join(".harness-sync/config.toml").exists());
}
