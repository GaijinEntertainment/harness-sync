use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

fn target_names(machine: &Machine) -> Vec<(String, bool)> {
    let (report, output) = machine.harness_sync_json(&["target", "list"]);
    assert_exit(&output, 0);
    report["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|target| {
            (
                target["name"].as_str().unwrap().to_owned(),
                target["primary"] == Value::Bool(true),
            )
        })
        .collect()
}

#[test]
fn an_added_target_is_listed_and_an_unreachable_one_is_not_added() {
    let primary = Remote::new();
    let mirror = Remote::new();
    let machine = Machine::new();
    assert_exit(
        &machine.harness_sync(&["init", &primary.url(), "--name", "github"]),
        0,
    );

    let added = machine.harness_sync(&["target", "add", "mirror", &mirror.url()]);
    let missing = machine.home().join("no-such-remote.git");
    let unreachable =
        machine.harness_sync(&["target", "add", "broken", &missing.to_string_lossy()]);

    assert_exit(&added, 0);
    assert_exit(&unreachable, 1);
    assert_eq!(
        target_names(&machine),
        [("github".to_owned(), true), ("mirror".to_owned(), false)]
    );
}

#[test]
fn a_target_name_is_letters_digits_dashes_and_underscores_but_not_all() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let all = machine.harness_sync(&["target", "add", "all", &remote.url()]);
    let spaced = machine.harness_sync(&["target", "add", "a b", &remote.url()]);

    assert_exit(&all, 2);
    assert_exit(&spaced, 2);
}

#[test]
fn only_a_target_that_is_not_the_primary_can_be_removed() {
    let primary = Remote::new();
    let mirror = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &primary.url()]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );

    let primary_removed = machine.harness_sync(&["target", "remove", "origin"]);
    let mirror_removed = machine.harness_sync(&["target", "remove", "mirror"]);

    assert_exit(&primary_removed, 1);
    assert!(text(&primary_removed.stderr).contains("primary target"));
    assert_exit(&mirror_removed, 0);
    assert_eq!(target_names(&machine), [("origin".to_owned(), true)]);
}

#[test]
fn a_new_primary_has_no_base_commit_so_each_differing_item_is_a_conflict() {
    let first_remote = Remote::new();
    let second_remote = Remote::new();
    let other = Machine::new();
    other.write_settings(&json!({"model": "sonnet"}));
    assert_exit(&other.harness_sync(&["init", &second_remote.url()]), 0);
    assert_exit(&other.harness_sync(&["push"]), 0);
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &first_remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "second", &second_remote.url()]),
        0,
    );

    let switched = machine.harness_sync(&["target", "primary", "second"]);

    assert_exit(&switched, 0);
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["base_commit"], Value::Null);
    let model = status["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "settings:/model")
        .unwrap();
    assert_eq!(model["state"], "conflict");
}

fn snapshot_settings(remote: &Remote) -> Value {
    serde_json::from_str(&remote.show("main:settings.json")).unwrap()
}

fn connected_with_mirror(primary: &Remote, mirror: &Remote) -> Machine {
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &primary.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );
    machine
}

#[test]
fn a_push_to_a_target_replaces_its_snapshot_and_keeps_the_base_commit() {
    let primary = Remote::new();
    let mirror = Remote::new();
    let other = Machine::new();
    other.write_settings(&json!({"model": "sonnet", "effortLevel": "high"}));
    assert_exit(&other.harness_sync(&["init", &mirror.url()]), 0);
    assert_exit(&other.harness_sync(&["push"]), 0);
    let machine = connected_with_mirror(&primary, &mirror);
    let primary_head = primary.head("main");

    let (report, output) = machine.harness_sync_json(&["push", "--to", "mirror"]);

    assert_exit(&output, 0);
    assert_eq!(snapshot_settings(&mirror), json!({"model": "opus"}));
    assert_eq!(primary.head("main"), primary_head);
    assert_eq!(report["pushes"][0]["target"], "mirror");
    assert_eq!(
        report["pushes"][0]["commit"].as_str(),
        mirror.head("main").as_deref()
    );
    let replaced: Vec<&Value> = report["pushes"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["id"])
        .collect();
    assert_eq!(replaced, ["settings:/effortLevel", "settings:/model"]);
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["base_commit"].as_str(), primary_head.as_deref());
}

#[test]
fn a_push_to_all_starts_with_the_primary_and_stops_when_the_primary_is_ahead() {
    let primary = Remote::new();
    let mirror = Remote::new();
    let machine = connected_with_mirror(&primary, &mirror);
    machine.write_settings(&json!({"model": "haiku"}));

    let (report, output) = machine.harness_sync_json(&["push", "--to", "all"]);

    assert_exit(&output, 0);
    let order: Vec<&Value> = report["pushes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|push| &push["target"])
        .collect();
    assert_eq!(order, ["origin", "mirror"]);
    assert_eq!(snapshot_settings(&primary), json!({"model": "haiku"}));
    assert_eq!(snapshot_settings(&mirror), json!({"model": "haiku"}));

    primary.commit_file("main", "notes.txt", "from another machine\n");
    machine.write_settings(&json!({"model": "opus"}));
    let mirror_head = mirror.head("main");

    let ahead = machine.harness_sync(&["push", "--to", "all"]);

    assert_exit(&ahead, 4);
    assert_eq!(mirror.head("main"), mirror_head);
}

#[test]
fn a_push_to_a_target_applies_the_exclude_list_of_that_target() {
    let primary = Remote::new();
    let mirror = Remote::new();
    mirror.commit_file(
        "main",
        "harness-sync.toml",
        "exclude = [\"settings:/model\"]\n",
    );
    let machine = connected_with_mirror(&primary, &mirror);
    machine.write_settings(&json!({"model": "opus", "effortLevel": "high"}));

    let output = machine.harness_sync(&["push", "--to", "mirror"]);

    assert_exit(&output, 0);
    assert_eq!(snapshot_settings(&mirror), json!({"effortLevel": "high"}));
    assert!(
        mirror
            .show("main:harness-sync.toml")
            .contains("settings:/model")
    );
}

#[test]
fn a_push_to_a_target_that_does_not_exist_fails() {
    let remote = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let output = machine.harness_sync(&["push", "--to", "nowhere"]);

    assert_exit(&output, 1);
    assert!(text(&output.stderr).contains("there is no target `nowhere`"));
}
