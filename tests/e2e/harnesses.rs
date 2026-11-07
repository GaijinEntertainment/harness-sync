use serde_json::json;

use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn a_machine_that_does_not_sync_a_harness_leaves_its_items_alone() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"model": "opus"}));
    first.write_file(".claude/rules/rust.md", "Use clippy.\n");
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::new();
    second.write_settings(&json!({"model": "sonnet"}));
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(
        &second.harness_sync(&["harness", "remove", "claude-code"]),
        0,
    );
    second.write_file(".agents/AGENTS.md", "Shared rules.\n");
    remote.commit_file("main", "harness-sync.toml", "include = [\".agents\"]\n");

    let (pulled, pull) = second.harness_sync_json(&["pull"]);
    let push = second.harness_sync(&["push"]);

    assert_exit(&pull, 0);
    let ids: Vec<&str> = pulled["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert_eq!(ids, [".agents/AGENTS.md"]);
    assert_eq!(second.settings(), json!({"model": "sonnet"}));
    assert_eq!(second.read_file(".claude/rules/rust.md"), None);
    assert_exit(&push, 0);
    assert!(remote.show("main:.claude/settings.json").contains("opus"));
    assert_eq!(remote.show("main:.claude/rules/rust.md"), "Use clippy.\n");
    assert_eq!(remote.show("main:.agents/AGENTS.md"), "Shared rules.\n");
}

#[test]
fn an_included_root_file_reaches_every_machine() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "include = [\".agents/skills\"]\n",
    );
    let first = Machine::new();
    first.write_file(".agents/skills/review/SKILL.md", "Review code.\n");
    first.write_file(".agents/other.md", "Not included.\n");
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["pull"]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::new();
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);

    let output = second.harness_sync(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        second
            .read_file(".agents/skills/review/SKILL.md")
            .as_deref(),
        Some("Review code.\n")
    );
    assert_eq!(second.read_file(".agents/other.md"), None);
}

#[test]
fn harness_add_and_remove_change_the_list_of_the_machine() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let removed = machine.harness_sync(&["harness", "remove", "claude-code"]);
    let (listed, _) = machine.harness_sync_json(&["harness", "list"]);
    let unknown = machine.harness_sync(&["harness", "add", "vim"]);
    let added = machine.harness_sync(&["harness", "add", "claude-code"]);

    assert_exit(&removed, 0);
    assert_eq!(
        listed["harnesses"],
        json!([{"name": "claude-code", "synced": false}])
    );
    assert_exit(&unknown, 1);
    assert!(text(&unknown.stderr).contains("there is no harness `vim`"));
    assert_exit(&added, 0);
    let (listed, _) = machine.harness_sync_json(&["harness", "list"]);
    assert_eq!(listed["harnesses"][0]["synced"], true);
}

#[test]
fn a_harness_added_after_the_last_pull_has_no_base_until_the_next_pull() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"model": "opus"}));
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::new();
    second.write_settings(&json!({"model": "sonnet"}));
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(
        &second.harness_sync(&["harness", "remove", "claude-code"]),
        0,
    );
    assert_exit(&second.harness_sync(&["pull"]), 0);
    assert_exit(&second.harness_sync(&["harness", "add", "claude-code"]), 0);

    let push = second.harness_sync(&["push"]);
    let (status, _) = second.harness_sync_json(&["status"]);
    let stopped = second.harness_sync(&["pull"]);
    let pulled = second.harness_sync(&["pull", "--prefer", "remote"]);
    let (pushed, _) = second.harness_sync_json(&["push"]);

    assert_exit(&push, 1);
    assert!(text(&push.stderr).contains("run `harness-sync pull` first"));
    assert_eq!(status["items"][0]["state"], "conflict");
    assert_exit(&stopped, 3);
    assert_exit(&pulled, 0);
    assert_eq!(second.settings(), json!({"model": "opus"}));
    assert_eq!(pushed["outcome"], "ok");
    assert!(remote.show("main:.claude/settings.json").contains("opus"));
}
