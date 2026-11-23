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
    let config: toml::Table = config.parse().unwrap();
    assert_eq!(config["format"].as_integer(), Some(1));
}

fn run_with_config(args: &[&str], rewrite: impl FnOnce(&str) -> String) -> std::process::Output {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    let config = machine.read_file(".harness-sync/config.toml").unwrap();
    machine.write_file(".harness-sync/config.toml", &rewrite(&config));
    machine.harness_sync(args)
}

#[test]
fn a_sync_configuration_of_a_newer_format_is_refused_by_each_command_that_uses_the_state() {
    let newer = |config: &str| {
        config
            .replace("format = 1", "format = 2")
            .replace("primary = \"origin\"", "primary = 1")
    };
    let commands: [&[&str]; 3] = [
        &["status"],
        &["retired", "keep"],
        &["secret", "set", ".claude/settings.json:/env/GITHUB_TOKEN"],
    ];

    for command in commands {
        let output = run_with_config(command, newer);

        assert_exit(&output, 1);
        assert!(
            text(&output.stderr).contains("has the format 2"),
            "{command:?} stderr: {}",
            text(&output.stderr)
        );
    }
}

#[test]
fn a_sync_configuration_without_a_format_has_the_first_format() {
    let output = run_with_config(&["status"], |config| config.replace("format = 1\n", ""));

    assert_exit(&output, 0);
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
