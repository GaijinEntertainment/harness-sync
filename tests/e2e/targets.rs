use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

fn target_names(machine: &Machine) -> Vec<(String, bool)> {
    let (report, output) = machine.cc_sync_json(&["target", "list"]);
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
        &machine.cc_sync(&["init", &primary.url(), "--name", "github"]),
        0,
    );

    let added = machine.cc_sync(&["target", "add", "mirror", &mirror.url()]);
    let missing = machine.home().join("no-such-remote.git");
    let unreachable = machine.cc_sync(&["target", "add", "broken", &missing.to_string_lossy()]);

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
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);

    let all = machine.cc_sync(&["target", "add", "all", &remote.url()]);
    let spaced = machine.cc_sync(&["target", "add", "a b", &remote.url()]);

    assert_exit(&all, 2);
    assert_exit(&spaced, 2);
}

#[test]
fn only_a_target_that_is_not_the_primary_can_be_removed() {
    let primary = Remote::new();
    let mirror = Remote::new();
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &primary.url()]), 0);
    assert_exit(
        &machine.cc_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );

    let primary_removed = machine.cc_sync(&["target", "remove", "origin"]);
    let mirror_removed = machine.cc_sync(&["target", "remove", "mirror"]);

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
    assert_exit(&other.cc_sync(&["init", &second_remote.url()]), 0);
    assert_exit(&other.cc_sync(&["push"]), 0);
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.cc_sync(&["init", &first_remote.url()]), 0);
    assert_exit(&machine.cc_sync(&["push"]), 0);
    assert_exit(
        &machine.cc_sync(&["target", "add", "second", &second_remote.url()]),
        0,
    );

    let switched = machine.cc_sync(&["target", "primary", "second"]);

    assert_exit(&switched, 0);
    let (status, _) = machine.cc_sync_json(&["status"]);
    assert_eq!(status["base_commit"], Value::Null);
    let model = status["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "settings:/model")
        .unwrap();
    assert_eq!(model["state"], "conflict");
}
