use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

#[test]
fn an_added_exclude_entry_reaches_the_repo_and_the_next_push_honors_it() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus", "effortLevel": "high"}));
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);

    let (report, output) =
        machine.cc_sync_json(&["declarations", "add", "exclude", "settings:/model"]);

    assert_exit(&output, 0);
    assert_eq!(
        report["declarations"]["exclude"],
        json!(["settings:/model"])
    );
    assert!(remote.show("main:cc-sync.toml").contains("settings:/model"));
    assert_exit(&machine.cc_sync(&["push"]), 0);
    let settings: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(settings, json!({"effortLevel": "high"}));
}

#[test]
fn the_push_after_a_secret_declaration_masks_the_value_in_the_repo() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"env": {"INTERNAL_URL": "https://ci.internal"}}));
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.cc_sync(&["push"]), 0);

    let declared = machine.cc_sync(&[
        "declarations",
        "add",
        "secrets",
        "settings:/env/INTERNAL_URL",
    ]);
    let pushed = machine.cc_sync(&["push"]);

    assert_exit(&declared, 0);
    assert_exit(&pushed, 0);
    let settings: Value = serde_json::from_str(&remote.show("main:settings.json")).unwrap();
    assert_eq!(
        settings,
        json!({"env": {"INTERNAL_URL": "<cc-sync secret settings:/env/INTERNAL_URL>"}})
    );
}

#[test]
fn a_change_of_one_list_keeps_the_other_lists_and_keys() {
    let remote = Remote::new();
    remote.commit_file(
        "main",
        "cc-sync.toml",
        "secrets = [\"settings:/env/INTERNAL_URL\"]\nexclude = [\"settings:/model\"]\nfuture = 1\n",
    );
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);

    let added = machine.cc_sync(&["declarations", "add", "include", "file:hooks"]);
    let removed = machine.cc_sync(&["declarations", "remove", "exclude", "settings:/model"]);

    assert_exit(&added, 0);
    assert_exit(&removed, 0);
    let file: toml::Table = remote.show("main:cc-sync.toml").parse().unwrap();
    assert_eq!(
        file,
        "future = 1\ninclude = [\"file:hooks\"]\nsecrets = [\"settings:/env/INTERNAL_URL\"]\n"
            .parse()
            .unwrap()
    );
}

#[test]
fn an_entry_that_is_already_there_makes_no_commit() {
    let remote = Remote::new();
    remote.commit_file("main", "cc-sync.toml", "exclude = [\"settings:/model\"]\n");
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &remote.url()]), 0);
    let head = remote.head("main");

    let output = machine.cc_sync(&["declarations", "add", "exclude", "settings:/model"]);

    assert_exit(&output, 0);
    assert!(text(&output.stdout).contains("nothing to change"));
    assert_eq!(remote.head("main"), head);
}

#[test]
fn show_lists_the_declarations_of_another_target() {
    let primary = Remote::new();
    let mirror = Remote::new();
    mirror.commit_file("main", "cc-sync.toml", "exclude = [\"settings:/model\"]\n");
    let machine = Machine::new();
    assert_exit(&machine.cc_sync(&["init", &primary.url()]), 0);
    assert_exit(
        &machine.cc_sync(&["target", "add", "mirror", &mirror.url()]),
        0,
    );

    let (report, output) = machine.cc_sync_json(&["declarations", "show", "--target", "mirror"]);

    assert_exit(&output, 0);
    assert_eq!(
        report["declarations"],
        json!({"target": "mirror", "secrets": [], "include": [], "exclude": ["settings:/model"]})
    );
}
