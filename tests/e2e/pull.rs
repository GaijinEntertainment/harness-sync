use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

fn connected(remote: &Remote, settings: &Value) -> Machine {
    let machine = Machine::new();
    machine.write_settings(settings);
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);
    machine
}

fn pushed(remote: &Remote, settings: &Value) -> Machine {
    let machine = connected(remote, settings);
    assert_exit(&machine.cc_sync(&["push"]), 0);
    machine
}

#[test]
fn pull_on_a_second_machine_writes_the_remote_keys() {
    let remote = Remote::new();
    pushed(&remote, &json!({"effortLevel": "high", "env": {"A": "1"}}));
    let second = connected(&remote, &json!({"model": "opus"}));

    let (report, output) = second.cc_sync_json(&["pull", "--prefer", "remote"]);

    assert_exit(&output, 0);
    assert_eq!(report["outcome"], "ok");
    assert_eq!(report["base_commit"], remote.head("main").unwrap().as_str());
    assert_eq!(
        second.settings(),
        json!({"model": "opus", "effortLevel": "high", "env": {"A": "1"}})
    );
    let (status, _) = second.cc_sync_json(&["status"]);
    assert_eq!(
        status["items"],
        json!([{"id": "settings:/model", "state": "local_change", "local": "opus", "remote": null}])
    );
}

#[test]
fn a_change_moves_from_one_machine_to_the_other_and_back() {
    let remote = Remote::new();
    let first = pushed(&remote, &json!({"effortLevel": "high"}));
    let second = connected(&remote, &json!({}));
    assert_exit(&second.cc_sync(&["pull"]), 0);
    second.write_settings(&json!({"effortLevel": "low", "env": {"B": "2"}}));
    assert_exit(&second.cc_sync(&["push"]), 0);

    let (report, output) = first.cc_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        first.settings(),
        json!({"effortLevel": "low", "env": {"B": "2"}})
    );
}

#[test]
fn a_key_changed_on_both_machines_stops_the_pull() {
    let remote = Remote::new();
    pushed(&remote, &json!({"effortLevel": "high"}));
    let second = connected(&remote, &json!({"effortLevel": "low"}));
    let before = second.settings_text();

    let (report, output) = second.cc_sync_json(&["pull"]);

    assert_exit(&output, 3);
    assert_eq!(report["outcome"], "conflicts");
    assert_eq!(
        report["items"],
        json!([{"id": "settings:/effortLevel", "state": "conflict", "local": "low", "remote": "high"}])
    );
    assert_eq!(second.settings_text(), before);
    assert_eq!(second.backups(), Vec::<String>::new());
}

#[test]
fn take_resolves_a_conflict_with_the_named_side() {
    let remote = Remote::new();
    pushed(&remote, &json!({"effortLevel": "high", "model": "opus"}));
    let second = connected(&remote, &json!({"effortLevel": "low", "model": "sonnet"}));

    let output = second.cc_sync(&[
        "pull",
        "--take",
        "settings:/effortLevel=remote",
        "--prefer",
        "local",
    ]);

    assert_exit(&output, 0);
    assert_eq!(
        second.settings(),
        json!({"effortLevel": "high", "model": "sonnet"})
    );
    assert_exit(&second.cc_sync(&["push"]), 0);
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(snapshot, json!({"effortLevel": "high", "model": "sonnet"}));
}

#[test]
fn take_for_an_item_without_a_conflict_is_an_error() {
    let remote = Remote::new();
    pushed(&remote, &json!({"effortLevel": "high"}));
    let second = connected(&remote, &json!({}));

    let output = second.cc_sync(&["pull", "--take", "settings:/effortLevel=local"]);

    assert_exit(&output, 1);
    assert!(!second.home().join(".cc-sync/backups").exists());
}

#[test]
fn pull_changes_only_the_bytes_of_the_changed_keys_and_keeps_a_backup() {
    let remote = Remote::new();
    let first = pushed(
        &remote,
        &json!({"keep": [1, 2], "env": {"A": "1", "B": "2"}, "model": "opus"}),
    );
    let second = Machine::new();
    let text = "{\n\t\"keep\":   [1,2],\n\t\"env\": {\"A\": \"1\", \"B\": \"2\"},\n\t\"model\": \"opus\"\n}\n";
    second.write_settings_text(text);
    assert_exit(&second.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&second.cc_sync(&["pull"]), 0);
    first.write_settings(&json!({"keep": [1, 2], "env": {"A": "1"}, "model": "sonnet"}));
    assert_exit(&first.cc_sync(&["push"]), 0);

    let output = second.cc_sync(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        second.settings_text(),
        "{\n\t\"keep\":   [1,2],\n\t\"env\": {\"A\": \"1\"},\n\t\"model\": \"sonnet\"\n}\n"
    );
    assert_eq!(second.backups(), [text]);
}

#[test]
fn pull_from_an_empty_remote_writes_nothing() {
    let remote = Remote::new();
    let machine = connected(&remote, &json!({"model": "opus"}));

    let (report, output) = machine.cc_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(machine.backups(), Vec::<String>::new());
}

#[test]
fn a_path_under_the_home_directory_follows_the_home_of_each_machine() {
    let remote = Remote::new();
    let first = Machine::new();
    let command =
        |machine: &Machine| format!("bash {}/.claude/statusline.sh", machine.home().display());
    first.write_settings(&json!({"statusLine": {"type": "command", "command": command(&first)}}));
    assert_exit(&first.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.cc_sync(&["push"]), 0);
    let second = connected(&remote, &json!({}));

    let output = second.cc_sync(&["pull"]);

    assert_exit(&output, 0);
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(
        snapshot["statusLine"]["command"],
        "bash <cc-sync home>/.claude/statusline.sh"
    );
    assert_eq!(second.settings()["statusLine"]["command"], command(&second));
    let (status, _) = first.cc_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn a_pinned_item_moves_in_neither_direction() {
    let remote = Remote::new();
    let first = pushed(&remote, &json!({"model": "opus", "effortLevel": "high"}));
    let second = connected(&remote, &json!({}));
    assert_exit(&second.cc_sync(&["pull"]), 0);
    assert_exit(&second.cc_sync(&["pin", "settings:/model"]), 0);
    second.write_settings(&json!({"model": "sonnet", "effortLevel": "low"}));
    assert_exit(&second.cc_sync(&["push"]), 0);
    first.write_settings(&json!({"model": "haiku", "effortLevel": "low"}));
    assert_exit(&first.cc_sync(&["pull"]), 0);
    assert_exit(&first.cc_sync(&["push"]), 0);

    let (report, output) = second.cc_sync_json(&["pull"]);

    assert_exit(&output, 0);
    assert_eq!(
        second.settings(),
        json!({"model": "sonnet", "effortLevel": "low"})
    );
    assert_eq!(
        report["items"],
        json!([{"id": "settings:/model", "state": "pinned", "local": "sonnet", "remote": "haiku"}])
    );
    let snapshot: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(snapshot, json!({"model": "haiku", "effortLevel": "low"}));
    assert_exit(&second.cc_sync(&["unpin", "settings:/model"]), 0);
    let (status, _) = second.cc_sync_json(&["status"]);
    assert_eq!(status["items"][0]["state"], "local_change");
}

#[test]
fn pin_refuses_a_key_path_that_is_not_an_item() {
    let remote = Remote::new();
    let machine = connected(&remote, &json!({}));

    assert_exit(&machine.cc_sync(&["pin", "settings:/env"]), 1);
    assert_exit(
        &machine.cc_sync(&["pin", "settings:/statusLine/command"]),
        1,
    );
}
