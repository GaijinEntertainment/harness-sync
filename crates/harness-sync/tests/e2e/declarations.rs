use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn an_added_exclude_entry_reaches_the_repo_and_the_next_push_honors_it() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus", "effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&[
        "declarations",
        "add",
        "exclude",
        ".claude/settings.json:/model",
    ]);

    assert_exit(&output, 0);
    assert_eq!(
        report["declarations"]["exclude"],
        json!([".claude/settings.json:/model"])
    );
    assert!(
        remote
            .show("main:harness-sync.toml")
            .contains(".claude/settings.json:/model")
    );
    assert_exit(&machine.harness_sync(&["push"]), 0);
    let settings: Value = serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
    assert_eq!(settings, json!({"effortLevel": "high"}));
}

#[test]
fn an_exclude_entry_removes_the_items_that_the_repo_already_holds() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus", "effortLevel": "high"}));
    machine.write_file(".claude/skills/old/SKILL.md", "Old skill.\n");
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);

    let output = machine.harness_sync(&[
        "declarations",
        "add",
        "exclude",
        ".claude/settings.json:/model",
        ".claude/skills/old",
    ]);

    assert_exit(&output, 0);
    let settings: Value = serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
    assert_eq!(settings, json!({"effortLevel": "high"}));
    let skill = remote.git(&["cat-file", "-e", "main:.claude/skills/old/SKILL.md"]);
    assert!(!skill.status.success());
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn the_push_after_a_secret_declaration_masks_the_value_in_the_repo() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"env": {"INTERNAL_URL": "https://ci.internal"}}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);

    let declared = machine.harness_sync(&[
        "declarations",
        "add",
        "secrets",
        ".claude/settings.json:/env/INTERNAL_URL",
    ]);
    let pushed = machine.harness_sync(&["push"]);

    assert_exit(&declared, 0);
    assert_exit(&pushed, 0);
    let settings: Value = serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
    assert_eq!(
        settings,
        json!({"env": {"INTERNAL_URL": "<harness-sync secret .claude/settings.json:/env/INTERNAL_URL>"}})
    );
}

#[test]
fn a_change_of_one_list_keeps_the_other_lists_and_keys() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "secrets = [\".claude/settings.json:/env/INTERNAL_URL\"]\nexclude = [\".claude/settings.json:/model\"]\nfuture = 1\n",
    );
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let added = machine.harness_sync(&["declarations", "add", "include", ".claude/hooks"]);
    let removed = machine.harness_sync(&[
        "declarations",
        "remove",
        "exclude",
        ".claude/settings.json:/model",
    ]);

    assert_exit(&added, 0);
    assert_exit(&removed, 0);
    let file: toml::Table = remote.show("main:harness-sync.toml").parse().unwrap();
    assert_eq!(
        file,
        "future = 1\ninclude = [\".claude/hooks\"]\nsecrets = [\".claude/settings.json:/env/INTERNAL_URL\"]\n"
            .parse()
            .unwrap()
    );
}

#[test]
fn an_entry_that_is_already_there_makes_no_commit() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        "exclude = [\".claude/settings.json:/model\"]\n",
    );
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    let head = remote.head("main");

    let output = machine.harness_sync(&[
        "declarations",
        "add",
        "exclude",
        ".claude/settings.json:/model",
    ]);

    assert_exit(&output, 0);
    assert!(text(&output.stdout).contains("nothing to change"));
    assert_eq!(remote.head("main"), head);
}

#[test]
fn show_lists_the_declarations_of_another_target() {
    let primary = Remote::new();
    let mirror = Remote::new();
    mirror.commit_file(
        "main",
        "harness-sync.toml",
        "exclude = [\".claude/settings.json:/model\"]\n",
    );
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &primary.url()]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );

    let (report, output) =
        machine.harness_sync_json(&["declarations", "show", "--target", "mirror"]);

    assert_exit(&output, 0);
    assert_eq!(
        report["declarations"],
        json!({"target": "mirror", "secrets": [], "include": [], "exclude": [".claude/settings.json:/model"]})
    );
}
