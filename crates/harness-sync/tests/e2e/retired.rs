use serde_json::json;

use crate::support::{Machine, Remote, assert_exit, text};

const MODEL: &str = ".claude/settings.json:/model";
const RULE: &str = ".claude/rules/old.md";

/// Two machines that hold a synced `model` key and a synced rule file, after the first one excluded both.
fn excluded_on_the_first() -> (Remote, Machine, Machine) {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"model": "opus", "effortLevel": "high"}));
    first.write_file(RULE, "old rule\n");
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::new();
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&second.harness_sync(&["pull"]), 0);
    assert_exit(
        &first.harness_sync(&["declarations", "add", "exclude", MODEL, RULE]),
        0,
    );
    (remote, first, second)
}

#[test]
fn pull_lists_and_warns_about_the_items_that_an_exclude_entry_retired() {
    let (_remote, _first, second) = excluded_on_the_first();

    let (before, _) = second.harness_sync_json(&["status"]);
    let pulled = second.harness_sync(&["pull"]);
    let (again, _) = second.harness_sync_json(&["pull"]);
    let (status, _) = second.harness_sync_json(&["status"]);

    assert_eq!(before.get("retired_items"), None);
    assert_exit(&pulled, 0);
    assert!(
        text(&pulled.stdout).contains(&format!("  retired         {MODEL}\n")),
        "stdout: {}",
        text(&pulled.stdout)
    );
    assert!(
        text(&pulled.stderr).contains("`harness-sync retired remove` deletes them here"),
        "stderr: {}",
        text(&pulled.stderr)
    );
    assert_eq!(again["retired_items"], json!([RULE, MODEL]));
    assert_eq!(status["retired_items"], json!([RULE, MODEL]));
    assert_eq!(second.settings()["model"], "opus");
    assert_eq!(second.read_file(RULE).as_deref(), Some("old rule\n"));
    let text_status = second.harness_sync(&["status"]);
    assert!(
        text(&text_status.stderr).contains("`harness-sync retired keep` keeps them"),
        "stderr: {}",
        text(&text_status.stderr)
    );
}

#[test]
fn a_pull_that_stops_on_a_conflict_still_lists_the_retired_items() {
    let (_remote, first, second) = excluded_on_the_first();
    assert_exit(&second.harness_sync(&["pull"]), 0);
    first.write_settings(&json!({"model": "opus", "effortLevel": "low"}));
    assert_exit(&first.harness_sync(&["push"]), 0);
    second.write_settings(&json!({"model": "opus", "effortLevel": "max"}));

    let (report, output) = second.harness_sync_json(&["pull"]);

    assert_exit(&output, 3);
    assert_eq!(report["retired_items"], json!([RULE, MODEL]));
}

#[test]
fn an_excluded_item_of_a_harness_added_after_the_last_pull_is_not_retired() {
    const THEME: &str = ".pi/agent/settings.json:/theme";
    let remote = Remote::new();
    let first = Machine::new();
    first.write_file(".pi/agent/settings.json", "{\"theme\": \"dark\"}\n");
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let second = Machine::new();
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&second.harness_sync(&["pull"]), 0);
    second.write_file(".pi/agent/settings.json", "{\"theme\": \"light\"}\n");
    assert_exit(&second.harness_sync(&["harness", "add", "pi"]), 0);
    assert_exit(
        &first.harness_sync(&["declarations", "add", "exclude", THEME]),
        0,
    );

    let (report, output) = second.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report.get("retired_items"), None);
    assert_eq!(
        second.read_file(".pi/agent/settings.json").as_deref(),
        Some("{\"theme\": \"light\"}\n")
    );
}

#[test]
fn an_item_whose_exclude_entry_is_removed_leaves_the_retired_items() {
    let (_remote, first, second) = excluded_on_the_first();
    assert_exit(&second.harness_sync(&["pull"]), 0);
    assert_exit(
        &first.harness_sync(&["declarations", "remove", "exclude", MODEL, RULE]),
        0,
    );

    let (status, output) = second.harness_sync_json(&["status"]);

    assert_exit(&output, 0);
    assert_eq!(status.get("retired_items"), None);
    assert!(
        !second
            .home()
            .join(".harness-sync/retired-items.toml")
            .exists()
    );
    assert_eq!(second.settings()["model"], "opus");
}

#[test]
fn the_machine_that_added_the_exclude_entry_has_no_retired_item() {
    let (_remote, first, _second) = excluded_on_the_first();

    let (status, output) = first.harness_sync_json(&["status"]);

    assert_exit(&output, 0);
    assert_eq!(status.get("retired_items"), None);
    assert_eq!(first.settings()["model"], "opus");
}

#[test]
fn retired_remove_deletes_the_retired_items_with_a_backup() {
    let (_remote, _first, second) = excluded_on_the_first();
    assert_exit(&second.harness_sync(&["pull"]), 0);
    let record = second.home().join(".harness-sync/retired-items.toml");
    assert!(record.is_file());

    let (report, output) = second.harness_sync_json(&["retired", "remove"]);

    assert_exit(&output, 0);
    assert!(!record.exists());
    assert_eq!(report["retired_items"], json!([RULE, MODEL]));
    assert_eq!(second.settings(), json!({"effortLevel": "high"}));
    assert_eq!(second.read_file(RULE), None);
    let backups = second.backup_files();
    assert!(backups.contains(&"old rule\n".to_owned()));
    assert!(backups.iter().any(|backup| backup.contains("\"opus\"")));
    let (status, _) = second.harness_sync_json(&["status"]);
    assert_eq!(status.get("retired_items"), None);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn retired_keep_leaves_the_items_and_stops_the_warning() {
    let (_remote, _first, second) = excluded_on_the_first();
    assert_exit(&second.harness_sync(&["pull"]), 0);

    let (report, output) = second.harness_sync_json(&["retired", "keep"]);

    assert_exit(&output, 0);
    assert!(
        !second
            .home()
            .join(".harness-sync/retired-items.toml")
            .exists()
    );
    assert_eq!(report["retired_items"], json!([RULE, MODEL]));
    assert_eq!(second.settings()["model"], "opus");
    assert_eq!(second.read_file(RULE).as_deref(), Some("old rule\n"));
    let (status, _) = second.harness_sync_json(&["status"]);
    assert_eq!(status.get("retired_items"), None);
    let (pulled, _) = second.harness_sync_json(&["pull"]);
    assert_eq!(pulled.get("retired_items"), None);
    for command in ["status", "pull"] {
        let output = second.harness_sync(&[command]);
        assert!(
            !text(&output.stderr).contains("retired"),
            "{command} stderr: {}",
            text(&output.stderr)
        );
    }
}

#[test]
fn an_item_that_the_user_deleted_by_hand_leaves_the_retired_items() {
    let (_remote, _first, second) = excluded_on_the_first();
    assert_exit(&second.harness_sync(&["pull"]), 0);
    std::fs::remove_file(second.home().join(RULE)).unwrap();

    let (status, output) = second.harness_sync_json(&["status"]);

    assert_exit(&output, 0);
    assert_eq!(status["retired_items"], json!([MODEL]));
    let record = second
        .read_file(".harness-sync/retired-items.toml")
        .unwrap();
    assert!(!record.contains(RULE), "record: {record}");
}

#[test]
fn an_excluded_item_that_never_synced_to_this_machine_is_not_retired() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "harness-sync.toml",
        &format!("exclude = [\"{MODEL}\"]\n"),
    );
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report.get("retired_items"), None);
    assert_eq!(machine.settings()["model"], "opus");
}
