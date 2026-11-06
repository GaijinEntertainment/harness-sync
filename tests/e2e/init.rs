use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn init_connects_to_an_empty_remote() {
    let remote = Remote::new();
    let machine = Machine::new();

    let (report, output) = machine.cc_sync_json(&["init", &remote.url()]);

    assert_exit(&output, 0);
    assert_eq!(report["command"], "init");
    assert_eq!(report["outcome"], "ok");
    assert!(report["remote_commit"].is_null());
    let config = std::fs::read_to_string(machine.home().join(".cc-sync/config.toml")).unwrap();
    assert!(config.contains(&remote.url()), "config: {config}");
}

#[test]
fn init_with_the_same_remote_again_succeeds() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);

    let output = machine.cc_sync(&["init", &remote.url()]);

    assert_exit(&output, 0);
}

#[test]
fn init_with_another_remote_is_refused() {
    let first = Remote::new();
    let second = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &first.url()]), 0);

    let output = machine.cc_sync(&["init", &second.url()]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("already connected"),
        "stderr: {}",
        text(&output.stderr)
    );
}

#[test]
fn init_with_an_unreachable_remote_fails_without_a_config() {
    let machine = Machine::new();
    let missing = machine.home().join("no-such-remote.git");

    let output = machine.cc_sync(&["init", &missing.to_string_lossy()]);

    assert_exit(&output, 1);
    assert!(
        text(&output.stderr).contains("git fetch failed"),
        "stderr: {}",
        text(&output.stderr)
    );
    assert!(!machine.home().join(".cc-sync/config.toml").exists());
}
