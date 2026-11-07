#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit, text};

const CHANGE_URL: &str = "https://gerrit.example/c/setup/+/42";

fn gerrit_remote() -> Remote {
    let remote = Remote::new();
    remote.commit_file("main", "README.md", "setup\n");
    let hook = remote.path().join("hooks/post-receive");
    fs::write(
        &hook,
        format!("#!/bin/sh\necho 'New Changes:'\necho '  {CHANGE_URL} harness-sync push [NEW]'\n"),
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    remote
}

fn change_ref(remote: &Remote) -> String {
    text(&remote.git(&["rev-parse", "refs/for/main"]).stdout)
        .trim()
        .to_owned()
}

fn has_change_id(remote: &Remote, commit: &str) -> bool {
    let message = text(&remote.git(&["log", "-1", "--format=%B", commit]).stdout);
    message.lines().any(|line| {
        line.strip_prefix("Change-Id: I")
            .is_some_and(|id| id.len() == 40 && id.chars().all(|c| c.is_ascii_hexdigit()))
    })
}

#[test]
fn a_gerrit_target_gets_a_change_for_review_and_its_branch_stays() {
    let primary = Remote::new();
    let review = gerrit_remote();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &primary.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert_exit(
        &machine.harness_sync(&["target", "add", "review", &review.url(), "--gerrit"]),
        0,
    );
    let branch_head = review.head("main");

    let (report, output) = machine.harness_sync_json(&["push", "--to", "review"]);

    assert_exit(&output, 0);
    assert_eq!(review.head("main"), branch_head);
    let change = change_ref(&review);
    assert_eq!(
        report["pushes"][0]["review"]["commit"],
        Value::String(change.clone())
    );
    assert_eq!(report["pushes"][0]["review"]["change_url"], CHANGE_URL);
    assert!(has_change_id(&review, &change));
}

#[test]
fn a_gerrit_primary_keeps_the_base_commit_until_the_change_merges() {
    let review = gerrit_remote();
    let machine = Machine::new();
    assert_exit(
        &machine.harness_sync(&["init", &review.url(), "--gerrit"]),
        0,
    );
    assert_exit(&machine.harness_sync(&["pull"]), 0);
    let base = review.head("main");
    machine.write_settings(&json!({"model": "opus"}));

    let sent = machine.harness_sync(&["push"]);

    assert_exit(&sent, 0);
    let (pending, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(pending["base_commit"].as_str(), base.as_deref());
    assert_eq!(pending["items"][0]["state"], "local_change");

    review.git(&["update-ref", "refs/heads/main", &change_ref(&review)]);
    assert_exit(&machine.harness_sync(&["pull"]), 0);

    let (merged, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(
        merged["base_commit"].as_str(),
        review.head("main").as_deref()
    );
    assert_eq!(merged["items"], json!([]));
}

#[test]
fn a_declarations_change_on_a_gerrit_target_goes_for_review() {
    let review = gerrit_remote();
    let machine = Machine::new();
    assert_exit(
        &machine.harness_sync(&["init", &review.url(), "--gerrit"]),
        0,
    );
    let branch_head = review.head("main");

    let (report, output) =
        machine.harness_sync_json(&["declarations", "add", "exclude", "settings:/model"]);

    assert_exit(&output, 0);
    assert_eq!(review.head("main"), branch_head);
    let change = change_ref(&review);
    assert!(has_change_id(&review, &change));
    assert!(
        review
            .show(&format!("{change}:harness-sync.toml"))
            .contains("settings:/model")
    );
    assert_eq!(report["pushes"][0]["review"]["change_url"], CHANGE_URL);
}
