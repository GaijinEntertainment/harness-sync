use serde_json::{Value, json};

use crate::support::{Machine, Remote, assert_exit};

#[test]
fn push_to_an_empty_remote_creates_the_first_remote_snapshot() {
    let remote = Remote::new();
    let machine = Machine::new();
    let settings = json!({"effortLevel": "high", "env": {"A": "1"}});
    machine.write_settings(&settings);
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(report["outcome"], "ok");
    assert_eq!(report["items"].as_array().unwrap().len(), 2);
    let head = remote.head("main").expect("the push created branch main");
    assert_eq!(report["remote_commit"], head.as_str());
    assert_eq!(report["base_commit"], head.as_str());
    let snapshot: Value = serde_json::from_str(&remote.show("main:.claude/settings.json")).unwrap();
    assert_eq!(snapshot, settings);
    let (status, _) = machine.harness_sync_json(&["status"]);
    assert_eq!(status["items"], json!([]));
}

#[test]
fn push_without_differences_creates_no_commit() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    let head = remote.head("main");

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 0);
    assert_eq!(report["items"], json!([]));
    assert_eq!(remote.head("main"), head);
}

#[test]
fn push_from_a_machine_that_has_not_pulled_is_refused() {
    let remote = Remote::new();
    let first = Machine::new();
    first.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&first.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&first.harness_sync(&["push"]), 0);
    let head = remote.head("main");
    let second = Machine::new();
    second.write_settings(&json!({"effortLevel": "low"}));
    assert_exit(&second.harness_sync(&["init", &remote.url()]), 0);

    let (report, output) = second.harness_sync_json(&["push"]);

    assert_exit(&output, 4);
    assert_eq!(report["outcome"], "remote_ahead");
    assert_eq!(
        report["items"],
        json!([{"id": ".claude/settings.json:/effortLevel", "state": "conflict", "local": "low", "remote": "high"}])
    );
    assert_eq!(remote.head("main"), head);
}

#[cfg(unix)]
#[test]
fn push_that_meets_a_commit_made_during_the_push_reports_its_items() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"effortLevel": "high"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert!(remote.git(&["branch", "ahead", "main"]).status.success());
    remote.commit_file("ahead", ".claude/rules/other.md", "Another machine.\n");
    machine.write_file(
        "hooks/pre-push",
        &format!(
            "#!/bin/sh\ngit --git-dir='{}' update-ref refs/heads/main refs/heads/ahead\n",
            remote.url()
        ),
    );
    machine.set_executable("hooks/pre-push");
    let config = machine.read_file(".gitconfig").unwrap();
    let hooks = machine.home().join("hooks");
    machine.write_file(
        ".gitconfig",
        &format!("{config}[core]\n\thooksPath = {}\n", hooks.display()),
    );
    machine.write_settings(&json!({"effortLevel": "low"}));

    let (report, output) = machine.harness_sync_json(&["push"]);

    assert_exit(&output, 4);
    assert_eq!(report["outcome"], "remote_ahead");
    assert_eq!(
        report["items"],
        json!([{"id": ".claude/settings.json:/effortLevel", "state": "local_change", "local": "low", "remote": "high"}])
    );
    assert_eq!(remote.head("main"), remote.head("ahead"));
}

#[test]
fn push_with_only_pinned_items_that_differ_says_so() {
    let remote = Remote::new();
    let machine = Machine::new();
    machine.write_settings(&json!({"model": "opus"}));
    assert_exit(&machine.harness_sync(&["init", &remote.url()]), 0);
    assert_exit(&machine.harness_sync(&["push"]), 0);
    assert_exit(
        &machine.harness_sync(&["pin", ".claude/settings.json:/model"]),
        0,
    );
    machine.write_settings(&json!({"model": "sonnet"}));

    let output = machine.harness_sync(&["push"]);

    assert_exit(&output, 0);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("nothing to push: only pinned items differ from the remote snapshot")
    );
}
